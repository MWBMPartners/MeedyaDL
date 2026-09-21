---
name: Multi-service expansion groundwork status
description: prep/expanded-services-groundwork branch state, plus the milestone renumbering that happened since the original plan
type: project
originSessionId: 2ab3d7da-8f4e-4331-8327-4ea82ab8e25f
---
The `prep/expanded-services-groundwork` branch (live on origin as of 2026-04-27) holds the foundational multi-service work for M8–M10. A duplicate `pre/expanded-services-groundwork` branch (typo) also exists on origin and should be deleted.

**Milestone renumbering vs. the original 2026-04-10 plan:**
- M8 (v2.0) — was Spotify, **now BBC iPlayer** (issue #102)
- M9 (v2.1) — was YouTube, **now Spotify** (issues #101, #110, #295)
- M10 (v2.2) — was BBC iPlayer, **now YouTube** (issues #103, #104)

CLAUDE.md's "Planned Service Integrations" section already reflects the new ordering. The old `project_meedyadl_v2_archive.md` memory still references the original mapping in passing — disregard that part.

**Branch contents (verified at the time the prep branch was last updated):**
- Backend modules registered & compiling: service_status, smart_download, service_dispatch, spotify_service, youtube_service, bbc_iplayer_service
- Backend models registered & compiling: service_status, content_match, download_options, votify_options, ytdlp_options, get_iplayer_options
- Frontend types: MediaServiceId, ServiceStatusConfig, QualityTier, CrossPlatformMatch, SmartDownloadResult
- IPC wrappers: checkServiceStatus(), checkCrossPlatform(), getServiceAuthStatus()
- UI: ServiceStatusBanner.tsx, SpotifyTab/YouTubeTab/BBCiPlayerTab placeholders, "Services" Settings group, DownloadForm "coming soon" message for non-Apple URLs

**NOT yet wired (deferred to whichever milestone lands first — BBC iPlayer M8):**
- commands/service_status.rs and commands/smart_download.rs not in `generate_handler!`
- Real subprocess integration for votify / yt-dlp / get_iplayer
- Per-service enrichment pipelines (only Apple Music has the full 12-stage pipeline)

**Why:** Foundation for the next major-version family. The branch was built ahead of the actual M8 work so the architecture would be settled before service-specific code lands.

**How to apply:** When starting M8 (BBC iPlayer), merge `prep/expanded-services-groundwork` into main first, then build on top. Verify the branch hasn't drifted from main before merging — it's been sitting since mid-April and `main` has moved through ~17 patch versions in that window. The `meedyadl-v2` archive branch was deleted; useful files were already extracted.

**Status check, 21 September 2026 (issue sweep, checked against the code on `alpha`).** The `prep/expanded-services-groundwork` branch is gone from the remote, so the "How to apply" advice above to merge it can no longer be followed. **Most of what it carried did reach `alpha` another way:** the six `services/*` modules, the `service_status`, `content_match`, `votify_options`, `ytdlp_options` and `get_iplayer_options` models, both command files, `ServiceStatusBanner.tsx` and the three service settings tabs are all present. **What never arrived:** `models/download_options.rs`, the four frontend types and the three frontend wrappers (`checkServiceStatus()`, `checkCrossPlatform()`, `getServiceAuthStatus()` do not exist in `tauri-commands.ts`), a BBC iPlayer entry in `PerServiceSettings` (#423), shared "which platforms use this engine" tracking (#424), and per-service sign-in status (#426) — see also #431. Also changed from what this file says: `commands/service_status.rs` and `commands/smart_download.rs` ARE registered in `lib.rs` today, but nothing in the frontend calls them, so the feature is still unreachable (#110, #106). And M9 (Spotify) is no longer a placeholder: the votify engine is switched on, the Spotify tab has real settings, and a Spotify download needs developer access and consent — see issue #101's 21 Sept comment for what is still not wired (the playback throttle and the pause between tracks).
