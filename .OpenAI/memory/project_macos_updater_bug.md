---
name: macOS in-app updater recurring issue
description: macOS updater has failed across multiple releases due to filename mismatches and pre-release endpoint issues
type: project
originSessionId: 3077ffe9-2509-4112-9e18-ef53b04ad9ea
---
The macOS in-app updater has been a recurring problem (#357, #368). Root cause identified 2026-04-10: Tauri 2.x names the updater bundle `MeedyaDL.app.tar.gz` (no arch suffix) but `release.yml` looked for `MeedyaDL_aarch64.app.tar.gz`. Fix in commit `4653cfc` on `optimisation/ram-usage-fix`.

Secondary issue: `tauri.conf.json` updater endpoint uses `/releases/latest/download/latest.json` which only resolves to non-prerelease releases. Since all releases are currently pre-releases, this 404s. However, the explicit "Download & Install" flow uses a tag-specific URL (`/releases/download/{tag}/latest.json`) so it works once `latest.json` has the `darwin-aarch64` entry.

**Why:** This has bitten the project at least twice. The filename mismatch is subtle because the build succeeds (warnings are non-fatal).

**How to apply:** After any release workflow changes, verify that `latest.json` contains a `darwin-aarch64` platform entry. If all releases remain pre-releases, the background Tauri updater check will not work — only the explicit download flow via `update_checker.rs` will detect updates.
