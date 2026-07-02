# GAMDL v3.8 Compatibility Audit

**Date**: 2026-07-02
**GAMDL release audited**: 3.8 (released 2026-06-29)
**Diff range**: `3.7.4..3.8` (7 commits, 10 files)
**Predecessor audit**: [`gamdl-v3.7.4-audit.md`](./gamdl-v3.7.4-audit.md)
**Tracking issue**: #962

## TL;DR

Not a pure zero-code-change admission. Six of the seven commits are internal to GAMDL's HLS pipeline / uploaded-video interface / date-tag helper / CLI warning copy, and admit cleanly. Two commits require MeedyaDL follow-through:

- **`a7d141b7` — new `/v1/play/assets` HLS endpoint.** GAMDL's non-wrapper HLS master-URL discovery moves off the song-metadata `extendedAssetUrls.enhancedHls` field and onto a new `POST /v1/play/assets` endpoint. The user-facing effect is that every non-web codec **except ALAC** now works without wrapper on 3.8+ (aac, aac-he, aac-binaural, aac-downmix, aac-he-binaural, aac-he-downmix, atmos, ac3). Companion commit `4d2988b3` narrows GAMDL's own CLI warning + README wording to say only ALAC is wrapper-dependent. MeedyaDL doesn't call `/v1/play/*` itself and doesn't parse m3u8 URLs, so this arrives on the HTTP / auth / parsing surfaces for free — but the downstream `SongCodec::is_wrapper_dependent()` filter (used only by gap-fill retry) and the `(Experimental)` labels on the codec dropdown are now conceptually stale on 3.8+. Not a correctness regression; filed as two `for consideration` follow-ups.

- **`58f4548` — `--no-exceptions` restored to effectiveness on 3.8+.** Silently a no-op on 3.1..3.7.4 because `structlog.processors.ExceptionPrettyPrinter()` was in the processor list unconditionally; 3.8 gates it on `not config.no_exceptions`. MeedyaDL currently drops the flag on **any** 3.1+ version via a direct `is_version_at_least("3.1")` check in `download_queue.rs::merge_options` (lines 3556-3560), so 3.8 users silently lose the upstream restoration. This is a genuine (if minor) regression against the upstream fix and warrants a real change: (a) tighten the `merge_options` predicate to reopen emission on >= 3.8, (b) refresh the `GamdlFeature::NoExceptionsFlag` gate predicate + doc comment to describe the three-era pattern (mirrors `FFmpegPath`), (c) add a test parameterised across 3.0 / 3.5 / 3.7.4 / 3.8. MeedyaDL's downstream `is_python_traceback_noise` filter (#660) already suppresses the noise, so this is UX / defence-in-depth, not urgent.

Also required: bump `maximum_tested_version` and `recommended_version` in [`tool-versions.toml`](../../src-tauri/tool-versions.toml) from `"3.7.4"` → `"3.8"`, with an audit-trail block matching the shape of the v3.7.4 entry.

Same overall shape as the v3.7.2 + v3.7.3 audit (#898) — bug-fix-heavy upstream release with one targeted MeedyaDL code change; not a pure ceiling bump like v3.3 / v3.5 / v3.5.1 / v3.5.2 / v3.7.4.

## Methodology

Identical to the v3.7.4 audit pattern. Six MeedyaDL integration surfaces cross-referenced against every upstream commit:

1. `src-tauri/src/models/gamdl_options.rs` — `to_cli_args`, INI emission gates, `SongCodec` traits (`is_wrapper_dependent`, `required_audio_trait`, `display_name`).
2. `src-tauri/src/services/config_service.rs` — `settings_to_ini`, `sanitize_ini_value`, `ini_metadata_section`, `ini_template_section`, `ini_audio_section`.
3. `src-tauri/src/services/download_queue.rs` — subprocess spawn + stdout/stderr readers + `extract_python_exception` + `merge_options` (including the direct `is_version_at_least` guards) + `build_gapfill_priority_chain` + completion task.
4. `src-tauri/src/services/gamdl_capabilities.rs` — `GamdlFeature` enum + `supports()` gates + `active_capabilities_summary()`.
5. `src-tauri/src/services/gamdl_service.rs` — `install_gamdl` (pip), `build_gamdl_command`, tool-path injection, `get_gamdl_version`.
6. `src-tauri/src/utils/process.rs` — regexes (`TRACK_INFO_V2_REGEX`, `ERROR_PREFIX_REGEX`, `PYTHON_EXCEPTION_REGEX`, `SAVED_REGEX`) + classifiers (`classify_error`, `is_io_error`, `is_python_traceback_noise`, `humanise_codec_skip_line`, `is_storefront_mismatch_error`, `is_media_not_streamable_error`).
7. `src-tauri/tool-versions.toml` — `[gamdl]` support window.

Plus the frontend `SongCodec::display_name` render site (`src/components/settings/AudioQuality.tsx`) and the wrapper-recommendation help text (`help/quality-settings.md`, `help/fallback-quality.md`) — both flagged as "completeness gaps" by the adversarial verifier and included here for full coverage.

## v3.8 — `3.7.4..3.8` change set

Seven commits:

| Commit | Subject | Files |
| --- | --- | --- |
| `a7d141b7` | Use assets API for song HLS streams | `gamdl/api/apple_music.py`, `gamdl/api/constants.py`, `gamdl/interface/song.py` |
| `d26b895b` | Fix uploaded video media handling | `gamdl/interface/interface.py`, `gamdl/interface/uploaded_video.py` |
| `4e97b0c4` | Respect album date when tagging songs | `gamdl/interface/song.py` |
| `8e3b94f6` | Fallback to release date for missing album dates | `gamdl/interface/base.py` |
| `4d2988b3` | Clarify ALAC wrapper warning | `README.md`, `gamdl/cli/cli.py` |
| `6cdbe015` | Bump version to 3.8 | `gamdl/__init__.py`, `pyproject.toml`, `uv.lock` |
| `58f4548` | Respect no exceptions option | `gamdl/cli/cli.py` |

### Finding 3.8-A — New `/v1/play/assets` HLS endpoint (lossy codecs unblocked wrapper-less)

`gamdl/api/constants.py` gains `APPLE_MUSIC_ASSETS_API_URI = "/v1/play/assets"`. `gamdl/api/apple_music.py::AppleMusicApi` gains a new `get_assets(media_id, kind="song", include_license_urls=True, hls_encryption="CBC", hls_profile="enhancedHls")` method that `POST`s to the new endpoint. `gamdl/interface/song.py` renames the private `_get_m3u8_master_url_from_metadata` → `_get_m3u8_master_url_from_assets` and rewrites it to call `get_assets(play_params.get("id") or song_metadata["id"], play_params.get("kind", "song"))` and pick the first asset's `url` field, instead of re-fetching the song metadata and reading `extendedAssetUrls.enhancedHls`.

`get_m3u8_master_url` still short-circuits through `_get_m3u8_from_playback(playback)` when the wrapper supplied a playback response, so the new assets path only fires on the non-wrapper code path — i.e., the change strictly unlocks previously-failing codecs, doesn't touch the wrapper flow.

**Why it matters user-side**: The old `extendedAssetUrls.enhancedHls` field was gated by Apple for aac / aac-he / aac-binaural / aac-downmix / aac-he-binaural / aac-he-downmix / atmos / ac3. Pre-3.8, MeedyaDL users on wrapper-less setups requesting any of those codecs would silently skip tracks. On 3.8+, the assets endpoint returns HLS masters for all of them; only ALAC remains gated (confirmed by the narrowed warning in commit `4d2988b3`).

**MeedyaDL surface impact — HTTP / auth / parsing / INI / templates**: none. Verified `apple_music_api.rs` calls only `api.music.apple.com/v1/catalog/*` + `itunes.apple.com/lookup` — never `/v1/play/*`, so no client-side collision. Verified no stdout/stderr log-line, exception-class, or template-variable change. Same zero-touch shape as v3.7.4's `_switch_m3u8_master_url_to_default` internal helper.

**MeedyaDL surface impact — codec traits / gap-fill filter / display labels**: two concept-drift items, neither correctness-affecting.

1. `SongCodec::is_wrapper_dependent()` at `src-tauri/src/models/gamdl_options.rs:217-219` is:

   ```rust
   pub const fn is_wrapper_dependent(&self) -> bool {
       matches!(self, Self::Atmos | Self::Ac3)
   }
   ```

   The doc comment above claims Atmos/AC3 use spatial audio API endpoints that don't fall back per-track without wrapper — true pre-3.8, no longer true on 3.8+. The single consumer is `build_gapfill_priority_chain()` at `download_queue.rs:898-917`, which strips wrapper-dependent codecs from the secondary gap-fill retry chain when wrapper is off. Adversarial-verifier analysis: on 3.8+ the filter is over-conservative but not harmful in practice — tracks for which Atmos would succeed in the primary pass do succeed there and don't need retry; tracks for which no Atmos variant exists on Apple's catalog would skip on retry for the same reason they skipped in pass 1. Meanwhile ALAC — which *does* still need wrapper on 3.8+ — is NOT stripped by the filter, so the filter's mental model was partly wrong pre-3.8 anyway. **Verdict**: no code change in this PR; file a `for consideration` issue to audit the whole gap-fill filter against the 3.8 reality (may involve stripping ALAC and un-stripping Atmos/AC3, or dropping the filter altogether).

2. `SongCodec::display_name()` at `src-tauri/src/models/gamdl_options.rs:262-276` labels Alac, Atmos, Ac3, AacBinaural, Aac, AacHe, AacDownmix, AacHeBinaural, and AacHeDownmix all as `"(Experimental)"`. On 3.8+ this is user-visible misinformation for every codec except ALAC. **Verdict**: no code change in this PR; file a `for consideration` issue to review the labels once 3.8 becomes the support-window floor (or add a version-aware transform in the React layer earlier if the noise becomes a support burden).

### Finding 3.8-B — Uploaded-video interface bug fixes

`gamdl/interface/interface.py::_get_uploaded_video_media` had two bugs from an earlier refactor:

* `async for media in self.music_video.get_media(media)` → `self.uploaded_video.get_media(media)` (was calling the wrong interface).
* `yield` → `yield media` (was yielding `None`).

`gamdl/interface/uploaded_video.py::get_media` had a third:

* `media.media_id = media["id"]` → `media.media_id = media.media_metadata["id"]` (was subscripting the dataclass as a dict → `TypeError`).

**Why it matters**: uploaded-video downloads (i.e. `/post/*` URLs and personal-library uploaded videos) never worked in v3.7.x — they either yielded `None` into the caller loop or raised `TypeError`.

**MeedyaDL surface impact: none.** MeedyaDL's URL parser doesn't route to uploaded-video specifically — such URLs reach GAMDL as opaque URL strings and either work or fall through to the `Unrecognised Apple Music URL shape` WARN log (#549). Post-3.8, uploaded-video URLs may now succeed rather than silently `TypeError`; if they don't succeed they'll surface via the existing `is_media_not_streamable_error` classifier bucket (#898) with the actionable "removed / region-locked / library-only" guidance. Zero-cost coverage improvement.

### Finding 3.8-C — `--use-album-date` now honoured for songs

`gamdl/interface/song.py::get_media` — both branches (playback-response present, and webplayback fallback) now pass `self.use_album_date` as the third positional arg to `self.base.get_tags_from_asset_info(...)`. Pre-3.8 those call sites called `get_tags_from_asset_info` with only two args, so `use_album_date` defaulted to `False` and the CLI flag silently had no effect on song downloads (only music videos honoured it).

Sibling commit `8e3b94f6` refactors `gamdl/interface/base.py::get_tags_from_asset_info` so a missing `playlistId` on the asset (uploaded videos, singles served without a playlist wrapper) no longer raises `KeyError` when `use_album_date=True` — falls through to `releaseDate` and emits a `no_playlist_id_for_album_date` debug log.

**MeedyaDL surface impact: none.** The `use_album_date: Option<bool>` field exists on `GamdlOptions` at `models/gamdl_options.rs:727` and is emitted unconditionally when `Some(true)` at line 1129, but no MeedyaDL setting wires it — it stays at its `None` default on every download regardless of GAMDL version. If MeedyaDL later surfaces this in Settings, no capability gate is needed (the flag has always existed at the CLI level; only its runtime effect was broken 2.x..3.7.4). The new `no_playlist_id_for_album_date` DEBUG log doesn't reach MeedyaDL's stdout/stderr readers under the default INFO log level.

### Finding 3.8-D — Narrowed ALAC-only wrapper warning + README rewrite

`gamdl/cli/cli.py`:

```python
# Before:
if any(not codec.is_web for codec in config.song_codec_piority) and not config.use_wrapper:
    logger.warning("You have chosen an experimental song codec without enabling wrapper. "
                   "They're not guaranteed to work due to API limitations.")

# After:
if SongCodec.ALAC in config.song_codec_piority and not config.use_wrapper:
    logger.warning("You have chosen ALAC without enabling wrapper. "
                   "ALAC may be attempted without wrapper, but it probably won't work due to API limitations.")
```

`README.md` collapses the "wrapper is recommended when using these non-web song codecs" bulleted list of 9 codecs to `The wrapper is recommended when using the alac song codec.` and retitles the corresponding quality-section subheader.

**Why it matters**: This is upstream's own confirmation that the assets-API change (commit `a7d141b7`) works wrapper-less for every non-web codec except ALAC. It doesn't change any CLI flag, INI key, wrapper protocol, or exception; the warning line's shape is still `[WARNING HH:MM:SS] <text>` which falls through to `GamdlOutputEvent::Unknown` in `parse_gamdl_output` (correct — it's neither an ERROR nor a per-track SKIP).

**MeedyaDL surface impact — parsing**: none. **MeedyaDL surface impact — traits/labels**: covered by the two `for consideration` follow-ups filed under Finding 3.8-A.

**MeedyaDL docs impact**: `help/quality-settings.md` and `help/fallback-quality.md` (any topic saying "wrapper required for non-web codecs" or "Dolby Atmos experimental") are conceptually stale for 3.8+ users. Not a release blocker; file as a docs-follow-up issue tied to the same 3.8 admission.

### Finding 3.8-E — `--no-exceptions` restored to effectiveness (structlog processor now conditional)

`gamdl/cli/cli.py`:

```python
# Before (3.1..3.7.4): ExceptionPrettyPrinter unconditional.
processors = [
    structlog.processors.add_log_level,
    structlog.processors.ExceptionPrettyPrinter(),
    custom_structlog_formatter,
]

# After (3.8+): gated on config.no_exceptions.
processors = [structlog.processors.add_log_level]
if not config.no_exceptions:
    processors.append(structlog.processors.ExceptionPrettyPrinter())
processors.append(custom_structlog_formatter)
```

**Why it matters user-side**: on 3.8+, `--no-exceptions` / `no_exceptions = true` actually suppresses the pretty-printed multi-line traceback again — the exception summary line remains, but the intervening `Traceback (most recent call last):` / `File "..."` frames disappear.

**MeedyaDL surface impact: BEHAVIOURAL REGRESSION AGAINST THE UPSTREAM FIX.** MeedyaDL currently drops the flag on any 3.1+ version via a direct version check in `download_queue.rs::merge_options`:

```rust
// download_queue.rs:3546-3560
if !settings.verbose_gamdl_exceptions {
    options.no_exceptions = Some(true);
}
// Drop the flag when we've positively detected a v3.1+ release —
// upstream ignores it there ...
if let Some(ver) = super::gamdl_capabilities::detected_version() {
    if super::gamdl_service::is_version_at_least(&ver, "3.1") {
        options.no_exceptions = None;
    }
}
```

The predicate is `>= 3.1`, so on 3.8 the field is still zeroed out — MeedyaDL never emits `--no-exceptions`, and 3.8 users silently lose the upstream fix even though the flag is again effective.

Additionally, the `GamdlFeature::NoExceptionsFlag` gate at `gamdl_capabilities.rs:557` is:

```rust
Self::NoExceptionsFlag => !is_version_at_least(version, "3.1"),
```

with a doc comment claiming "effective on v2.x and v3.0 only … no effect on output" from 3.1+ onward. The gate is **not consumed by `to_cli_args()`** — emission at `models/gamdl_options.rs:1181-1183` is unconditional on `self.no_exceptions == Some(true)`. The comment at `download_queue.rs:3541-3545` incorrectly claims the gate does the gating; the actual gating is the inline version check above. This is stale on both counts on 3.8+.

**MeedyaDL surface impact — parsing**: net-positive noise reduction *if* the flag reaches emission — MeedyaDL's `is_python_traceback_noise` filter (#660) and the `TracebackFrame` variant were designed to swallow exactly the frames the flag now suppresses upstream. The two layers stack cleanly (upstream flag OR downstream filter suppresses noise; the exception summary line is preserved and still hits `PYTHON_EXCEPTION_REGEX` → `GamdlOutputEvent::Error`). Zero regex change needed.

**Required fix** (see Actions section for full detail):

1. Update the `merge_options` predicate to `is_version_at_least(&ver, "3.1") && !is_version_at_least(&ver, "3.8")` (or, preferably, route the decision through `supports(GamdlFeature::NoExceptionsFlag)` so the gate is the single source of truth).
2. Update `GamdlFeature::NoExceptionsFlag` predicate to `!is_version_at_least(version, "3.1") || is_version_at_least(version, "3.8")` — mirrors the three-era `FFmpegPath` pattern already in the file.
3. Rewrite the doc comment on the variant to describe all three eras: (a) `< 3.1`: effective (original), (b) `3.1..3.7.4`: no-op (upstream `dc6f2e8` removed consumers), (c) `>= 3.8`: effective again (upstream `58f4548` conditionally re-added `ExceptionPrettyPrinter`).
4. Update the `no_exceptions_flag_is_effective_below_v31` test at `gamdl_capabilities.rs:709` to also assert `false` on 3.7.4 and `true` on 3.8, or add a paired `no_exceptions_flag_reinstated_in_v38` test in the `FFmpegPath` three-era style at lines 1042-1086. Add a `merge_options` test that pins `no_exceptions` after merge on 3.0 / 3.5 / 3.7.4 / 3.8 to cover all four eras.

### Finding 3.8-F — Version bump

`gamdl/__init__.py`, `pyproject.toml`, `uv.lock` — `3.7.4` → `3.8`. Pure metadata. `pyproject.toml`'s `requires-python` did **not** change, so `install_gamdl` / `pip_version_spec` / `pip_target_spec` in `services/gamdl_service.rs` work unchanged.

## MeedyaDL surface impact summary

| Surface | v3.8 verdict |
| --- | --- |
| `src-tauri/src/models/gamdl_options.rs` (CLI encoding + traits) | No code change required for admission. Two `for consideration` items filed: `SongCodec::is_wrapper_dependent()` conceptually stale on 3.8+, and `SongCodec::display_name()` `(Experimental)` labels stale on 3.8+. |
| `src-tauri/src/services/config_service.rs` (INI emission) | Zero surface change. No new / removed / renamed INI keys, no default flips. |
| `src-tauri/src/services/download_queue.rs` (subprocess + `merge_options`) | **Action required.** Direct-version guard at lines 3556-3560 must permit `--no-exceptions` emission on >= 3.8. |
| `src-tauri/src/services/gamdl_capabilities.rs` (`GamdlFeature` gates) | **Action required.** `NoExceptionsFlag` predicate + doc comment need the three-era pattern. Consider consuming the gate from `merge_options` (single source of truth). |
| `src-tauri/src/services/gamdl_service.rs` (install pipeline, version detection) | Zero surface change. `pip_version_spec` / `pip_target_spec` operate on the new ceiling without modification. |
| `src-tauri/src/utils/process.rs` (regex + classifier) | Zero surface change. Log-line prefixes, exception classes, TRACK_INFO_V2_REGEX / ERROR_PREFIX_REGEX / PYTHON_EXCEPTION_REGEX / SAVED_REGEX inputs unchanged. |
| `src-tauri/tool-versions.toml` (support window) | **Action required.** Ceiling + recommended bump to `3.8`. |
| Frontend `SongCodec::display_name` render (`src/components/settings/AudioQuality.tsx`) | Filed as `for consideration` — label review once 3.8 is the support-window floor. |
| Help docs (`help/quality-settings.md`, `help/fallback-quality.md`, wrapper-recommended copy) | Filed as `for consideration` — docs drift for 3.8+ users. |

## Compatibility with the rest of the support window (3.5.2 → 3.7.4)

All three finding-driven changes preserve behaviour on every prior release in the support window:

- **`merge_options` predicate change**: The tightened predicate `is_version_at_least(ver, "3.1") && !is_version_at_least(ver, "3.8")` remains `true` for 3.1..3.7.4 (unchanged behaviour there — still zero out) and becomes `false` on 3.8+ (permits emission). Pre-3.1 continues to fall through unmodified.
- **`NoExceptionsFlag` gate predicate change**: `!is_version_at_least(version, "3.1") || is_version_at_least(version, "3.8")` matches the `FFmpegPath` three-era template already in the file. No other consumer of the gate exists today, so no downstream call site changes semantics unintentionally.
- **`tool-versions.toml` bump**: ceiling raise only; `minimum_version = "2.9.1"` unchanged, so no earlier release is dropped.

## Actions required

Blocker / High:

1. Bump `maximum_tested_version` and `recommended_version` in `src-tauri/tool-versions.toml` `[gamdl]` block from `"3.7.4"` → `"3.8"`, with a `# 3.8 audit` block appended to the audit-trail comment (see `tool_versions_toml_bump` for the diff).
2. Fix the `--no-exceptions` regression:
   - Update `merge_options` at `src-tauri/src/services/download_queue.rs:3556-3560` to gate on both floor **and** ceiling, or (preferred) route through `supports(GamdlFeature::NoExceptionsFlag)` and delete the inline check.
   - Update `GamdlFeature::NoExceptionsFlag` predicate at `src-tauri/src/services/gamdl_capabilities.rs:557` to `!is_version_at_least(version, "3.1") || is_version_at_least(version, "3.8")`.
   - Rewrite the doc comment on the variant (`gamdl_capabilities.rs:395-407`) to describe all three eras.
   - Extend the existing test `no_exceptions_flag_is_effective_below_v31` (`gamdl_capabilities.rs:709`) and/or add a new `no_exceptions_flag_reinstated_in_v38` in the `FFmpegPath` three-era style.
   - Add a `merge_options`-level test asserting `no_exceptions` reaches emission on 3.0, is dropped on 3.5.2 / 3.7.4, and reaches emission again on 3.8.

Medium (follow-up issues, do NOT block the 3.8 admission):

3. `SongCodec::is_wrapper_dependent()` review — audit the gap-fill filter against 3.8's ALAC-only reality; decide whether to invert (strip ALAC, keep Atmos/AC3), leave as-is, or introduce a `GamdlFeature::AssetsApiUnlocksLossyCodecs` gate at `>= 3.8`. See GitHub issue proposal below.
4. `SongCodec::display_name()` `(Experimental)` label review — plan for the 3.8-floor future; possibly a runtime-aware transform in the React layer earlier.
5. Help doc drift — audit `help/quality-settings.md`, `help/fallback-quality.md`, and any topic referring to "wrapper required for non-web codecs" or "Dolby Atmos experimental" for 3.8+ users.

Low (not blocking, no code change):

6. Cross-project note: v3.8 adds ~1 extra HTTPS round-trip per song on wrapper-less setups (the new `/v1/play/assets` call). Album downloads make `N` extra calls to Apple's edge. Users on rate-limited or metered connections may see minor effects. GAMDL 3.2 already narrowed default concurrency 5→1 as its own rate-limit reliability fix, so this is unlikely to trip Apple's throttles — flagged for the record.

## Out-of-scope / not changed

- `src-tauri/src/services/config_service.rs` — no INI-emission surface change. Verified `no_exceptions` is not written to INI on any path (CLI-arg-only), so the finding 3.8-E fix is scoped to `download_queue.rs` + `gamdl_capabilities.rs`.
- Wrapper triangle (`wrapper_account_url` + `wrapper_m3u8_ip` + `wrapper_decrypt_ip`, and `wrapper_url` for v3.6+) — protocol unchanged. `health_check_service.rs` preflight probes still valid.
- Apple Music API client (`apple_music_api.rs`) — MeedyaDL never calls `/v1/play/*`. Home-page token extraction (v3.7.4 delta), MusicKit JWT / Music-User-Token resolution, storefront rewriting/fallback all unchanged.
- Smart re-download / library scan (`services/smart_retry_planner.rs`) — the planner keys off codec IDs, not GAMDL feature availability; no version-aware change needed. A user upgrading from 3.7.4 → 3.8 who re-runs Re-download on an album whose Atmos / AC3 companion tier was previously skipped may now see those tiers succeed. Zero MeedyaDL code change; free coverage improvement.
- `services/gamdl_service.rs` install pipeline — no Python-version bump; `pip install --upgrade 'gamdl>={min},<={max}'` and `pip install --upgrade 'gamdl=={target}'` operate on the new ceiling without modification.
- Frontend URL parser + `URL audit diagnostics` (#487 umbrella) — no v3.8 commit touches GAMDL's `VALID_URL_PATTERN` or storefront handling.

