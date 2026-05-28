# GAMDL v3.7.2 + v3.7.3 audit

**Audit date:** 2026-05-28
**Releases audited:**

- [GAMDL 3.7.2](https://github.com/glomatico/gamdl/releases/tag/3.7.2) — 2026-05-28
- [GAMDL 3.7.3](https://github.com/glomatico/gamdl/releases/tag/3.7.3) — 2026-05-28

**MeedyaDL tracking issue:** #898
**Audit method:** raw commit patches (`https://github.com/glomatico/gamdl/commit/<sha>.patch`) + cross-check of `gamdl/interface/exceptions.py` and `gamdl/interface/types.py` at refs `3.5.2` / `3.7.1` / `3.7.2` / `3.7.3` to confirm the surface change is only what the patches show.

## Upstream changes

### 3.7.2 (3 commits)

| SHA       | Message                                  | Files touched              | Functional? |
| --------- | ---------------------------------------- | -------------------------- | ----------- |
| `817479d` | Use uncensored names and add sort fields | `interface/music_video.py` | yes         |
| `f54ab12` | Guard playParams access to avoid KeyError | `interface/song.py`        | yes         |
| `c6bce4b` | Bump version to 3.7.2                    | `pyproject.toml`           | no          |

### 3.7.3 (2 commits)

| SHA       | Message                              | Files touched              | Functional? |
| --------- | ------------------------------------ | -------------------------- | ----------- |
| `8398d9c` | Handle missing playParams in metadata | `interface/music_video.py` | yes         |
| `d88dbe5` | Bump version to 3.7.3                | `pyproject.toml`           | no          |

Total functional diff across both releases: **3 commits, ~4 lines changed.**

## Per-commit analysis

### `817479d` — music-video tag swap (3.7.2)

```diff
-            title=lookup_metadata[0]["trackCensoredName"],
+            title=lookup_metadata[0]["trackName"],
+            title_sort=lookup_metadata[0]["trackCensoredName"],
…
-            tags.album = lookup_metadata[1]["collectionCensoredName"]
+            tags.album = lookup_metadata[1]["collectionName"]
+            tags.album_sort = lookup_metadata[1]["collectionCensoredName"]
```

`title_sort` and `album_sort` map to the standard mp4 atoms `sonm` and `soal` respectively (`interface/types.py:74-96`). MeedyaDL does **not** write either atom in its enrichment pipeline (verified: `rg -n 'sonm|soal|title_sort|album_sort' src-tauri/src/` returns no hits in tag-emission code), so we don't collide with GAMDL's writes.

User-visible effect:

- Music-video filename templates containing `{title}` or `{album}` produce **uncensored** filenames after upgrading.
- Re-downloading a previously-fetched MV with `overwrite=false` (e.g. via Library Scan's smart-retry) will create a **new file** with a different filename rather than skipping. This is GAMDL's behaviour, not MeedyaDL's.
- Audio songs are unaffected — this change is in `music_video.py` only.

**Action:** documentation only. No MeedyaDL code change.

### `f54ab12` + `8398d9c` — defensive `playParams` access (3.7.2 songs, 3.7.3 music-videos)

Both commits are the same one-line defensive change in different files:

```diff
-        if media.media_metadata["attributes"]["playParams"].get("isLibrary"):
+        if media.media_metadata["attributes"].get("playParams", {}).get("isLibrary"):
```

Before the fix, songs / music-videos missing `playParams` raised a bare `KeyError: 'playParams'` traceback from GAMDL's exception printer, which MeedyaDL's `process::classify_error` falls through to `unknown`. After the fix, the same items reach the existing `if not self.base.is_media_streamable(...)` check (`song.py:520`, `music_video.py:460`) and raise the structured `GamdlInterfaceMediaNotStreamableError("Media is not streamable: <media_id>")`.

The error class itself is **not new** — `git show 3.5.2:gamdl/interface/exceptions.py` confirms `GamdlInterfaceMediaNotStreamableError` and its `"Media is not streamable: {media_id}"` message string have existed for at least four prior releases. What's new is that the message now surfaces *reliably* for affected items instead of being masked by the upstream KeyError.

MeedyaDL impact: the new bare error string did not match any of `auth` / `network` / `codec` / `not_found` / `rate_limit` / `tool`. It fell through to `unknown`, leaving users with the generic "Check the Activity Log" guidance.

**Action:** add `is_media_not_streamable_error()` helper + a dedicated `media_not_streamable` classifier bucket with actionable guidance ("removed / region-locked / library-only — try a different URL or storefront"). Ordered ahead of the broader `not_found` substring check so the bucket is preferred over a generic `not found` family match.

## MeedyaDL changes landed in this audit

| Change                                                           | File                                  |
| ---------------------------------------------------------------- | ------------------------------------- |
| `is_media_not_streamable_error()` helper                         | `src-tauri/src/utils/process.rs`      |
| `classify_error()` adds `media_not_streamable` bucket            | same                                  |
| `error_guidance()` adds friendly message for the new bucket      | same                                  |
| 6 new tests for matcher, classifier ordering, and guidance       | `process::tests`                      |
| Support window: `maximum_tested_version` 3.7.1 → 3.7.3           | `src-tauri/tool-versions.toml`        |
| `recommended_version` 3.7.1 → 3.7.3                              | same                                  |
| Audit-notes block for `[gamdl]` 3.7.2 + 3.7.3                    | same                                  |
| CLAUDE.md GAMDL cadence paragraph extended                       | `.claude/CLAUDE.md`                   |

## Compatibility with ≤3.5.x

- The classifier improvement is keyed on the literal error string `Media is not streamable`. On 3.5.x and earlier, the same string appears whenever GAMDL hits the existing `is_media_streamable` check on its own (e.g. for region-locked songs that *do* have `playParams`), so the new bucket benefits older releases too.
- No `GamdlFeature` gate needed. No CLI / INI / wrapper-protocol changes. The audit follows the same zero-or-minimal-code-change admission pattern as v3.3 / v3.5 / v3.5.1 / v3.5.2 / v3.7.1.

## Out-of-scope / not changed

- MV title/album field swap (Finding A in #898) — purely user-visible, no MeedyaDL code change needed.
- Settings schema (no new keys).
- Wrapper-v2 surface (untouched in both releases).
- `aac-web` codec rename (predates these releases).
