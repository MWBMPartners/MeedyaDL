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

## Status

The synthesised tests added in #615 cover the documented v3.2 output shapes
(conditional `Downloading` line + `FlatFilterExcluded` rename) based on the
upstream source-tree diff between the `3.1` and `3.2` tags. Real-sample
captures are a follow-up strengthening step once someone with a live
GAMDL-equipped environment runs the scenarios above.
