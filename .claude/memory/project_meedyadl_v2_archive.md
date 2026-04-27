---
name: meedyadl-v2 branch — historic context only
description: The meedyadl-v2 branch was deleted in April 2026; live multi-service prep is on prep/expanded-services-groundwork
type: project
originSessionId: 2ab3d7da-8f4e-4331-8327-4ea82ab8e25f
---
The `meedyadl-v2` branch was an early multi-service refactoring attempt that diverged too far for cherry-picking. It was deleted in April 2026 after useful files were extracted via `git show`.

**Files that were recovered** (originally to a `prep/refactoring/supported-service-expansion` working branch which was later consolidated into `prep/expanded-services-groundwork`):
- Per-engine CLI option models: `votify_options.rs`, `ytdlp_options.rs`, `get_iplayer_options.rs`
- Smart Download: `content_match.rs`, `smart_download.rs` (service + command)
- Service Status kill-switch: `service_status.rs` (model + service + command), `serviceStatusStore.ts`

**NOT recovered (superseded by main's #107 multi-service architecture):** URL parser, MediaServiceId enum, i18n, code formatting.

**Why:** The branch had useful code that would otherwise have been lost; dropping it without extracting first would have meant re-implementing those modules from scratch when M8–M10 work begins.

**How to apply:** This memory is now historic — the live multi-service prep is on `prep/expanded-services-groundwork` (see the multi-service groundwork memory). Don't reference `prep/refactoring/supported-service-expansion` in suggestions; that working branch no longer exists. The recovered modules are already integrated into the live prep branch and registered in `mod.rs`.
