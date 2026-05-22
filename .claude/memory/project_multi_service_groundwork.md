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
