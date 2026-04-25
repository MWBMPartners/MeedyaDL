# GAMDL v3.2 real-sample fixtures

This directory holds captured `stderr` / `stdout` from real GAMDL v3.2
runs, used by the regression tests in `src-tauri/src/utils/process.rs`
(see #615).

## How to capture

Run GAMDL v3.2 against a representative URL and redirect both streams:

```bash
python -m gamdl \
  --no-config-file \
  --log-level INFO \
  <URL> \
  > stdout.log 2> stderr.log
```

Commit the captured files here under a descriptive name, e.g.:

- `album-multi-track-stderr.log`
- `single-song-stderr.log`
- `music-video-stderr.log`
- `playlist-stderr.log`
- `artist-bucket-stderr.log`

## Redaction

Strip any personally-identifiable fragments before committing:

- wrapper URLs (may contain auth tokens)
- cookie values
- user home paths (replace with `$HOME`)
- account identifiers in AMP API responses

## Committed fixtures

The `.log` files in this directory are **synthesised** from the v3.2 upstream
source (specifically `gamdl/cli/utils.py::custom_structlog_formatter` which
owns the exact `[LEVEL    HH:MM:SS] [action] message` shape, and `cli.py`
which owns the conditional `Downloading "…"` emission and the per-track
`Track {index:>3}/{total:<3}` bracket padding). They capture the
**structural** invariants MeedyaDL's parser depends on, so the tests in
`src-tauri/src/utils/process.rs::tests::v32_fixture_*` assert counter values
and event types rather than exact whitespace.

| File | Scenario |
| --- | --- |
| `album-multi-track-stderr.log` | 3-track album with `[Track 1/3]` → `[Track 3/3]` |
| `single-song-stderr.log` | Single-song URL (`?i=…`), fires exactly one `[Track 1/1]` |
| `music-video-stderr.log` | Catalog MV URL (`music-video/…`), fires exactly one `[Track 1/1]` |
| `playlist-stderr.log` | 4-track playlist, `[Track 1/4]` → `[Track 4/4]` |
| `flat-filter-excluded-stderr.log` | WARNING line for the renamed `GamdlInterfaceFlatFilterExcludedError` class (verifies it doesn't falsely parse as `TrackInfo`) |

## Drop-in replacement workflow

When someone with a live v3.2 environment captures real output:

1. Redact per the checklist above.
2. Replace the synthesised file in place — keep the filename identical so the
   existing fixture-driven tests continue to pass.
3. Run `cargo test -p meedyadl --lib -- v32_fixture_` to confirm the
   assertions still hold. Since the tests target structural properties
   (counter values, event counts), real captures should pass without any
   test changes. If they don't, the mismatch is a genuine parser bug worth
   investigating — file it as a follow-up to #615.

## Status

- ✅ Synthesised fixtures + fixture-driven tests — committed.
- ⚠ Real-sample replacements — deferred, requires a live GAMDL v3.2
  environment.
