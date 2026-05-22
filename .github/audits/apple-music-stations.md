# Apple Music Radio show episode URLs (#804) — Phase 0 audit

**Date:** 2026-05-18
**Status:** investigation complete; awaiting upstream GAMDL support
**Issue:** [#804](https://github.com/MWBMPartners/MeedyaDL/issues/804)

## TL;DR

MeedyaDL cannot support `/station/.../ra.NNNNN` URLs (Apple Music on-demand
radio show episodes) on its own. The required URL regex addition + media
fetch dispatch live **upstream in GAMDL**. The upstream regex on `main` does
not accept `station` URLs; an open upstream issue
([glomatico/gamdl#281](https://github.com/glomatico/gamdl/issues/281)) has
been waiting since 2026-03-19 with our test URLs supplied.

**Decision:** defer. When upstream ships station support in any GAMDL
release within our support window, this issue moves to Phase 2 (MeedyaDL
parser/regex/template work).

## Upstream verification (via `gh api`, per the
[MeedyaSuite-core online-only standing rule](../../.claude/memory/project_meedyasuite_core_online_only.md))

GAMDL's `VALID_URL_PATTERN` on `main` at the time of this audit
(`gamdl/interface/constants.py`):

```python
VALID_URL_PATTERN = re.compile(
    r"https://(?:classical\.)?music\.apple\.com"
    r"(?:"
    r"/(?P<storefront>[a-z]{2})"
    r"/(?P<type>artist|album|playlist|song|music-video|post)"
    r"(?:/(?P<slug>[^\s/]+))?"
    r"/(?P<id>[0-9]+|pl\.[0-9a-z]{32}|pl\.u-[a-zA-Z0-9]+)"
    r"(?:\?i=(?P<sub_id>[0-9]+))?"
    r"|"
    r"(?:/(?P<library_storefront>[a-z]{2}))?"
    r"/library/(?P<library_type>playlist|albums)"
    r"/(?P<library_id>p\.[a-zA-Z0-9]+|l\.[a-zA-Z0-9]+)"
    r")"
)
```

Accepted entity types: `artist | album | playlist | song | music-video | post`,
plus the library branch (`playlist | albums`). **No `station` branch.**

Also confirmed via the contents listing of
`gh api repos/glomatico/gamdl/contents/gamdl/interface`: no `station.py` or
`radio.py` module exists. The dispatch in `interface.py` would have nothing
to call even if the regex matched.

## Upstream issue state

- [`glomatico/gamdl#281`](https://github.com/glomatico/gamdl/issues/281) — **OPEN**, created 2026-03-19, last activity 2026-05-12 (our follow-up comment from `Salem874` adding four more test URL examples). No maintainer response yet.
- No related PRs (open or closed) in `glomatico/gamdl` proposing station handling.

## Apple Music catalog API shape (still unverified)

The issue's Phase 0 also asks: what does
`/v1/catalog/{sf}/stations/{ra.…}` actually return? This audit does NOT
answer that — confirming the response schema needs a live MusicKit JWT
hit, which is best done as part of the upstream PR work (since the
response shape drives both the regex granularity and the media fetch
dispatch). Recording the question here so the next contributor doesn't
have to rediscover it.

## What MeedyaDL pre-work would look like (when upstream ships)

The path forward (per the issue's Phase 2 + Phase 3 + Phase 4) is well
scoped:

1. **Backend** ([`apple_music_api.rs`](../../src-tauri/src/services/apple_music_api.rs)):
   add `STATION_RE` regex matching `/(?:classical(?:\.music)?|music)\.apple\.com/[a-z]{2}/station/[^/]+/ra\.\d+`. Extend `parse_apple_music_url` with a new variant. Add `GamdlFeature::StationDownload` to [`gamdl_capabilities.rs`](../../src-tauri/src/services/gamdl_capabilities.rs) gated on the upstream release that ships the feature.
2. **Templates** ([`download_queue.rs`](../../src-tauri/src/services/download_queue.rs)):
   `STATION_FOLDER_TEMPLATE = "{station_name}"` and `STATION_FILE_TEMPLATE = "{episode_title} (ra.{id})"`. Follows the same Tier 4 safety-net pattern as `MV_NO_ALBUM_FOLDER_TEMPLATE` (#531).
3. **Frontend** ([`url-parser.ts`](../../src/lib/url-parser.ts) + [`types/index.ts`](../../src/types/index.ts)):
   add `'station'` to `AppleMusicContentType` + new `/station/` branch in `detectContentType`. Vitest cases alongside the existing 6 content-type tests.
4. **Engine config** ([`engines.toml`](../../src-tauri/engines.toml)):
   add `"radio-stations"` to Apple Music's `content_types`.
5. **Docs**: README "Supported URL types" + `help/downloading-radio.md` (new) or `help/downloading-music.md` (extension).
6. **Enrichment short-circuit**: station episodes don't have ISRC / per-track Apple Music metadata, so the 12-stage enrichment pipeline should bail out early via a content-type guard in `download_queue.rs::enrich_album_metadata`.

## Out of scope (per the issue spec)

- Live Apple Music 1 / Hits / Country live feeds (rolling stream, no fixed asset).
- Tracklist extraction + per-song chapter splitting of DJ mixes.
- Changes to GAMDL's URL handling for any other entity type.

## Next checkpoint

Re-run this audit when:
- A new GAMDL release ships within our `[minimum, maximum_tested]` window (currently 2.9.1 → 3.5.2 per [`tool-versions.toml`](../../src-tauri/tool-versions.toml)) AND its release notes mention station / radio support.
- Or `glomatico/gamdl#281` moves from `open` → `closed` with a referenced PR.

`gh issue view 281 --repo glomatico/gamdl` is the one-line check.
