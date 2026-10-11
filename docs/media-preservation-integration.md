# MeedyaDL: media preservation integration requirements

Status: proposed development/discovery requirements, 2026-10-11. No implemented support is claimed and existing milestones remain unchanged.

Use the [common media preservation and presentation contract](https://github.com/MeedyaSuite/MeedyaSuite-specs/blob/docs/cross-app-media-contract/standards/CROSS-APP-MEDIA-CONTRACT.md) alongside the [Converter research notes](https://github.com/MWBMPartners/MeedyaConverter/blob/docs/mp4-remux-preservation/docs/MP4-Remux-Preservation-Plan.md). These links reference draft working branches. Adopt a reviewed, commit-pinned contract before implementation/conformance claims; app-specific deviations need an explicit decision and capability report.

## Acquisition and post-processing requirements

Preserve every selected acquired representation through download/remux/enrichment, including native CEA-608 and CEA-708, HDR/Dolby Vision/HDR10+, stereo/multiview, audio configuration, offsets/priming, languages/roles, resources and relationships. An engine's default stream selection or subtitle-exclusion setting must not silently drop native captions. Caption exclusion requires an explicit service/track/standard decision.

Keep downloaded original metadata and Kodi XML/NFO/companions alongside normalized tags, source/provider provenance and checksums. Preserve unknown fields where safe. Lyrics conversion and subtitle generation are separate transformations; native captions must not automatically pass through a TTML/SRT/ASS/mov_text conversion pipeline.

Treat engine/backend/version capabilities as evidence, not guarantees from a file extension. Show preservation previews and unsupported selected features before remux/enrichment. A successful acquisition exit is not completion until required post-processing and verification finish. Retain sources on malformed trailer/configuration, cancellation or full disk.

Keep intentional codec companion tiers/compatibility alternatives even if their programme content matches. Configurable deduplication is disabled/recommend-only/opt-in proven consolidation, with separate quality policy and protected options; similarity/fingerprints alone cannot discard companions or caption services. No acquisition policy should implicitly authorize conversion losses.

## Discovery and acceptance

Exercise caption-bearing acquisitions through all post-processing stages, deliberately malformed codec configuration, missing services, metadata-only enrichment, cancellation during companions, fallback/codec tiers and multiple editions. Verify full native preservation independently of progress signals and sample-only scans. Record exact engine versions/effective commands without cookies or credentials.

## Planning integration

Carry these cases into future implementation issues and regression fixtures. Inventory/preview, safe edits and evidence contracts come first; edition-aware synchronization and incremental edits are bounded investigations. Do not infer support from a successful mux, codec label, progress message or an archived tag. Structural/sample checks, exhaustive preservation and target playback are different verification levels.
