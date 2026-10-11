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

## Caption naming, authoring and subtitle appearance follow-up

Follow the shared contract's [caption labels and explicit native authoring](https://github.com/MeedyaSuite/MeedyaSuite-specs/blob/docs/cross-app-media-contract/standards/CROSS-APP-MEDIA-CONTRACT.md#caption-labels-and-explicit-native-authoring) and [subtitle appearance/bitmap exploration](https://github.com/MeedyaSuite/MeedyaSuite-specs/blob/docs/cross-app-media-contract/standards/CROSS-APP-MEDIA-CONTRACT.md#subtitle-appearance-track-wide-placement-and-animated-bitmap-conversion). Proposed requirements include native 608/708 track/service naming, explicit subtitle-to-native-caption creation, whole-track PGS/DVB-Sub positioning and animated bitmap reconstruction. They do not claim implemented support or a delivery milestone.

Retain bold/italic/underline, colour/alpha, positioning, regions, fonts, outline/shadow, layering and dynamic styling whenever representable. Preserve exact originals and report source/target profile limits before conversion. Bitmap appearance is not semantic styling: OCR/style/animation recovery needs confidence and correction. Do not silently flatten animated captions or infer forced/SDH semantics from a display title.

Carry the practical native-caption investigation into fixtures: video-carried discovery beyond stream lists; both 608 fields and all 708 services; field-2-only samples; explicit inert packaging padding when required; evidenced source-to-presentation offsets; video association and caption ordering separate from defaults; independent payload/event readback. A backend's copy option is not proof of preservation. Full native 708 writing and Apple playback remain unverified.

The metadata-only naming trial verified a c608 handler label and existing object-name field while preserving media bytes, other tracks and caption sample tables. It did not validate universal title precedence or 708/player display. Prefer proven native naming for the chosen MP4/MOV profile; resolve localized tnam, object names and handler labels separately. Embedded services without a portable native label require a disclosed application/custom override, not silent stream splitting. Stable IDs, language, role, standard and service numbers survive renames.

Expose opt-in generation/embedding of CEA-608/708 from acquired SRT/SubRip, SSA/ASS or TTML/IMSC, with requested service/language/mode, label and target profile. Delegate to Converter and Subtitler's capable authoring/correction path; download success cannot mask failed post-processing. Preserve acquired rich sources, native captions and bitmap animation. Apply PGS/DVB positioning or conversion only under a selected policy, with per-feature losses and source provenance. Do not silently create 608 compatibility data, duplicate services or replace native captions. Encoder documentation demonstrates feasibility, not an installed/capable backend.

Acceptance backlog: Unicode/localized/empty caption labels, multiple services, naming precedence and all supported layouts; no-op/idempotence and failed-write rollback; source-style/profile mismatches; generated native captions with bandwidth/character/overlap limits; both fields/services and independent seek/render checks; whole-track PGS/DVB offsets including clipping/shared resources; palette-only animation, karaoke sweeps and manual correction. Use synthetic/licensed fixtures; supplied animation examples and output-player tests remain future work.

### Service inventory and display-mode UX

One c608 container track can expose CC1/CC2 in field 1 and CC3/CC4 in field 2; MediaInfo service entries are not necessarily extra physical tracks. A generic trial exposed two different languages under one track-level English label. Keep service-level language/name evidence separate from inherited track tags, never infer language or duplicates from channel number, and preserve both. Explain pop-on (prepare off-screen then reveal a block), paint-on (write into displayed text) and roll-up (append/scroll lines). “Type of the first event” describes only that observation; modes can change and startup punctuation need not represent later dialogue. Preserve state, preloading, clear/display timing, styles and seek restoration. 708 services/windows and carried 608 compatibility data are separate identities; native 608 inside digital carriage is not automatically native 708. Follow the [shared service/display-mode requirements](https://github.com/MeedyaSuite/MeedyaSuite-specs/blob/docs/cross-app-media-contract/standards/CROSS-APP-MEDIA-CONTRACT.md#caption-tracks-services-and-display-modes), including explanatory UI and mode-switch/multilingual fixtures.

### Multilingual caption selection

Track titles must summarize all evidenced languages; keep track labels and per-service language/role evidence separate. A multilingual native language value (where supported) does not select a service. Player chooses a physical track then a 608 channel or 708 service using manual choice or validated language/role mapping, with explicit unknown/default fallback. CC1/CC2 share field 1; CC3/CC4 share field 2, and none prescribes a language. 708 service numbers 1–63 are separate identifiers. Text services/XDS are not ordinary dialogue. Respect descriptor-specific meaning: ATSC A/65:2013 does not give its language field meaning for 608 digital_cc=0. Keep inspected/user-labelled language evidence and unknown/conflicting values; do not silently infer or invent a portable map. See [shared language-selection requirements](https://github.com/MeedyaSuite/MeedyaSuite-specs/blob/docs/cross-app-media-contract/standards/CROSS-APP-MEDIA-CONTRACT.md#language-selection-and-multilingual-track-naming). Include bilingual, descriptor-conflict, empty-service and unsupported-engine fixtures.

### Default native-language caption titles

Use `<Native language name 1> / <Native language name 2> / … [CC]` for new caption authoring or explicit rename/normalization, for example `English / Español [CC]`. Use accurate Unicode autonyms, not anglicized names/transliteration. List evidenced languages once in stable service/source order; retain region/script distinctions, unknown/conflicting states, original titles and user overrides. Untouched remuxes preserve existing names. Keep BCP 47/native codes and service mappings separate from display labels. Test accents, RTL/mixed scripts, encoding/length limits, deterministic order and idempotence. Follow the [shared naming convention](https://github.com/MeedyaSuite/MeedyaSuite-specs/blob/docs/cross-app-media-contract/standards/CROSS-APP-MEDIA-CONTRACT.md#default-caption-title-convention); it is a Meedya UX policy, not an upstream naming mandate.

### Tracked feature/discovery backlog

- [Opt-in subtitle-to-CEA-608/708 generation and service-aware post-processing](https://github.com/MWBMPartners/MeedyaDL/issues/1323)

## Ancillary data preservation and discovery roadmap (2026-10-11)

Preserve source data by default, subject to explicit user selection and verified target carriage. This includes both CEA-608 fields, CC1–CC4, text services T1–T4, XDS, CEA-708 services, Teletext (subtitle and general pages), and embedded programme/service/guide information from DVB and ATSC. Treat other or future formats as typed extensible records; do not assume an unidentified “DVG” label identifies a specific standard.

Field 2 is a transport distinction, not a subtitle language: retain its caption/text data and XDS without filtering it merely because Field 1 supplies the selected dialogue service. T1–T4 are text-mode services, distinct from CC1–CC4 caption services. XDS is ancillary data rather than a dialogue subtitle. Preserve native commands, payloads, field/channel/service IDs, sequence, timing, parity where applicable, associations and descriptors. Selecting no conventional subtitle tracks must not silently discard embedded ancillary data. A video transcode requires explicit extraction/reinsertion verification; stream copying alone is not evidence of complete retention.

Teletext preservation includes page/subpage identities, magazine, character sets, national options, control codes, colour/style/layout, updates, language evidence and relevant signalling. Distinguish subtitle pages from general information pages. DVB-SI and ATSC PSIP guide/service records require identity, table/version context, event timing and time references, original descriptors and provenance; record-derived metadata must not overwrite user or catalogue metadata silently.

Expose per-class and per-service opt-in/opt-out controls, with preservation enabled by default where supported, and explicit export/sidecar or alternate-container choices where necessary. MP4/MOV is not presumed to have a portable native mapping for every broadcast table or service. A sidecar/custom archival payload must be labelled as such; it does not constitute interoperable native playback. Never report full preservation when unsupported data was dropped. Keep an original/raw representation alongside any derived interpretation when feasible. Unknown data may only be copied with its necessary context; refuse unsafe relocation rather than invent semantics.

Every app must separately report identify, read/inspect, preserve, author/create, render/display and use capabilities by format/version/backend/platform. Add safe extension points for future types; unknown semantics are neither executable nor automatically authorable. Creation is an explicit action using a verified standards/profile-aware writer, with loss preview and validation. Parse source text and metadata as untrusted data, without executing links or actions.

MeedyaPlayer should offer readable language/service/page choices and optional text/guide views while retaining advanced inspection. MeedyaLibrary and MeedyaManager should index available services, raw versus interpreted guide metadata, provenance and capability limits, without hiding preserved data from inventory or treating broadcast claims as verified catalogue identity. Converter/Subtitler own explicit transformation/authoring controls; DL delegates appropriately; core supplies extensible models and policy; DB persists identities and evidence. These requirements apply suite-wide even if a backend cannot yet render or author a given type.

Acceptance fixtures: Field-2-only captions; mixed captions/text/XDS; multiple Teletext pages/subpages and character sets; DVB EIT/SDT/time and ATSC PSIP guide records; unknown descriptors; transcode versus remux; selective exclusion; unsupported MP4 carriage; raw round-trip verification, timing/service identity checks and readable nontechnical UI. Include malformed-input limits and clear partial-preservation reports.

Research references (not implementation or certification claims): [Goldman/TANDBERG 2008, sections 4.1–4.3](https://www.nab.org/documents/resources/paperGoldman.pdf), an engineering white paper; [ETSI EN 300 468 V1.18.1 (2023-12)](https://www.etsi.org/deliver/etsi_en/300400_300499/300468/01.18.01_60/en_300468v011801p.pdf), published DVB-SI reference including EIT and Teletext signalling. This bounded reference does not assert it is the latest edition. Implementation must pin the actual applicable Teletext, XDS, PSIP and carriage specifications and verify their profiles before claiming support.

### Ratings and content advisories

Preserve and explicitly author ratings with their scheme, region/jurisdiction, rating value, content dimensions/descriptors, event/programme scope, timing, original raw representation, authority/provenance and user-edit history. TV-MA is not a universal numeric age, and audience-measurement ratings are a different concept. Do not silently translate rating schemes or make unverified source labels authoritative. Keep conflicting XDS, broadcast guide, file-tag and catalogue values distinguishable; selecting a parental-control policy is separate from preserving metadata.

For ATSC 1.0, PSIP event Content Advisory Descriptors reference rating dimensions defined by Rating Region Tables: see [A/65:2013, sections 6.4 and 6.9.3](https://www.atsc.org/wp-content/uploads/2021/04/A65_2013.pdf). XDS is another carriage for content advisories, not an interchangeable authority for all digital receiver decisions. DVB parental-rating descriptors have their own country-specific semantics under EN 300 468. File-level rating tags/custom metadata can retain a declared rating but do not automatically retain native broadcast signalling or activate a player's parental controls. ATSC 3.0 and other systems require separate profile research.

ELI5: preserve the programme's information card as well as its picture and sound. A television can read the card to show a guide or a rating; a muxer can lose the card even when the movie looks unchanged. A label painted into the picture is separate from machine-readable rating data.

Illustrative uses: CC2/CC4 can carry additional or alternative caption services; their IDs do not mandate a language or special purpose. T1–T4 can provide general text information pages. XDS has practical uses for programme information and V-chip content advisories. These are format/use categories, not claims that any named broadcaster routinely populated every slot. Add verified real-world fixtures before claiming individual channel prevalence.

### Dynamic aspect ratio and active-picture preservation

Retain timed AFD (Active Format Description), bar data and applicable aspect/display signalling, including changes within a video stream. AFD is not a universal “IMAX switching” flag: it describes active-picture/display treatment; changing encoded geometry or sample/display aspect signalling, timed crop information and changing baked-in letterbox/pillarbox pixels are distinct mechanisms. Do not collapse a timeline into a single container-level aspect ratio or infer authorial intent from detected black bars.

Remuxing must verify applicable native signalling survives; transcoding must inspect, carry or explicitly transform per-frame/per-sequence metadata with correct timing and geometry. Resize/crop operations may invalidate source AFD/bar coordinates: recompute under an explicit validated policy or disclose loss, never blindly copy stale values. Preserve actual picture pixels by default; automatic crop/zoom is opt-in. Verify transitions with representative frames and metadata before/at/after boundaries, including subtitle placement and stereoscopic geometry.

Player should respect applicable signalling under a user-selectable fit/crop policy and avoid unwanted zoom; Library/Manager should inventory dynamic ranges/events rather than report one misleading value. Core/DB must store temporal geometry and provenance; Converter/Subtitler/DL must coordinate transformations. Backend/container support and authoring are individually verified, not implied by static metadata tags.

Primary research: [ATSC A/53 Part 4:2009, video user-data AFD and bar-data syntax/semantics](https://www.atsc.org/wp-content/uploads/2021/04/A_53-Part-4-2009.pdf). This is a bounded reference, not a latest-edition or cross-codec support claim.

### Supporting sources and examples

- [Apple QuickTime closed-caption media](https://developer.apple.com/documentation/quicktime-file-format/closed_captioning_media), [track names](https://developer.apple.com/documentation/quicktime-file-format/track_name_atom), and [user-data atoms](https://developer.apple.com/documentation/quicktime-file-format/user_data_atoms): caption carriage, relationships and naming research; distinguish QuickTime from ISO BMFF conventions.
- [ATSC A/65:2013](https://www.atsc.org/wp-content/uploads/2021/04/A65_2013.pdf), sections 6.9.2 and 6.9.3: service descriptors and content advisories; 608 language interpretation limitations remain important.
- [Sony XDS/text troubleshooting](https://www.sony.com/electronics/support/televisions-projectors/kd-36xs955/articles/00025071): manufacturer example of programme/station information, clock settings and V-chip ratings; explains the unexpected black text box.
- [Sony caption-service selector](https://helpguide.sony.net/vpl/phz51/v1/en/contents/TP1000768662.html): real product support for CC1–CC4 and Text1–Text4; evidence of receiver options, not proof that every service is broadcast.
- [Cablecast 608 versus 708](https://support.cablecast.tv/s/articles/captions-608-vs-708-caption-encodes-with-cablecast): vendor explanation of shared field bandwidth and common CC1/CC3 primary/secondary use; not a mandated language assignment.
- [GStreamer timed text to 608](https://gstreamer.freedesktop.org/documentation/rsclosedcaption/tttocea608.html) and [708](https://gstreamer.freedesktop.org/documentation/rsclosedcaption/tttocea708.html): concrete authoring feasibility examples; plugin limitations do not define format limits or select a project dependency.
- [Aegisub ASS tags](https://aegisub.org/docs/latest/ass_tags/), [TTML2 animation](https://www.w3.org/TR/ttml2/#animation-vocabulary), [IMSC 1.2 supported features](https://www.w3.org/TR/ttml-imsc1.2/#supported-features-and-extensions), and [ETSI EN 300 743 V1.6.1 (2018-10)](https://www.etsi.org/deliver/etsi_EN/300700_300799/300743/01.06.01_60/en_300743v010601p.pdf): style/animation and DVB subtitle conversion research; target profile and renderer limits must be tested.

Living web documentation is research observed on 2026-10-11, not a pinned implementation baseline. Exact source/version/status and conformance fixtures must be recorded when adopting a writer or parser. These links supplement the existing contract references and do not imply all features are implemented.

### Source-derived caption display modes

Prefer preservation of the source's native caption display commands and their timeline, including Pop-on, Paint-on, Roll-up, clears, memory swaps, positioning and mode changes. Never assign a whole-track mode from a probe's “first event” label. Native remuxing preserves command semantics; conversion preserves them where a faithful target representation exists and reports any approximation. Text sources without equivalent commands require an explicit configurable authoring policy; do not claim a generated mode was retained from the source.

### User caption presentation preferences

MeedyaPlayer and other rendering applications should offer source presentation by default, plus persistent user appearance preferences (font/size, colours, opacity/background, edge treatment and placement) where format/backend capabilities allow. Provide a clear source/default reset, per-profile settings, readable previews, and disclosure of unsupported overrides. Keep native service selection/language separate from presentation preferences. Playback overrides do not rewrite stored caption commands or mux metadata.

Preserve caption timing, mode changes and semantic layout when applying appearance preferences where possible. Forcing Pop-on/Paint-on/Roll-up or other temporal behaviour is a separate opt-in advanced transformation, with capability limits and warnings about altered presentation; it cannot be promised by every backend. Avoid obscuring important picture content or losing speaker/position distinctions without user awareness. Authoring/export edits in Converter/Subtitler remain explicit and show what is changed.
