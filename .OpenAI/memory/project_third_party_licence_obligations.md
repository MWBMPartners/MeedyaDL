---
type: project
title: Third-party licence obligations in MeedyaDL's bundle chain
---

# Third-party licence obligations in MeedyaDL's bundle chain

When a release is built with `bundle_engines=true` (offline-installer mode in
`release.yml` Step 8.5, ~lines 384–520), we redistribute third-party engines
and tools inside our signed installer artefacts. The obligation that attaches
depends on the upstream licence — not all of them are MIT-equivalent.

The default tag-push release path (`bundle_engines=false`) ships only
MeedyaDL's own code; engines and tools are pulled onto the end-user's machine
from PyPI / GitHub Releases at first launch, so the redistribution obligations
below do **not** attach to that variant. We still ship the upstream notices
with both variants for clarity and downstream-fork friendliness.

## Component / licence / obligation matrix

| Component | Licence | Obligation when bundled in the offline installer |
|---|---|---|
| GAMDL | MIT | Preserve copyright + permission notice. |
| votify | MIT | Same. |
| Bento4 / mp4decrypt | MIT | Same. |
| N_m3u8DL-RE | MIT | Same. |
| MediaInfo | BSD-2-Clause | Preserve copyright + notice (permissive, MIT-equivalent). |
| Python / python-build-standalone | PSF | Preserve notice. |
| **FFmpeg** | **LGPL-2.1+** | Preserve notice **+ written offer for source** + user must be able to substitute their own build (subprocess-invocation satisfies the substitutability requirement; we already do this). |
| **MP4Box / GPAC** | **LGPL-2.1** | Same as FFmpeg. |
| **get_iplayer** (planned, M8 / v2.0) | **GPL-3.0** | Preserve notice **+ provide the complete corresponding source** for get_iplayer itself (typically by shipping the unmodified upstream tarball alongside the binary, or by a written offer valid for three years). MeedyaDL's own MIT code is protected by GPL's "mere aggregation" exception because we subprocess-invoke get_iplayer — we do **not** link it — so the GPL does **not** propagate into MeedyaDL. |
| yt-dlp | Unlicense (public domain) | No legal obligation; still credit it. |

## What "written offer for source" means in practice

For each LGPL/GPL component shipped inside an offline installer, the bundle
must include:

1. A `LICENSE` (or `COPYING`) file alongside the binary, with the unmodified
   upstream text.
2. An `OFFER_FOR_SOURCE.txt` (or per-component fragment under
   `bundled-deps/source-offers/`) that states:
   - The component name + the **exact version** we shipped.
   - A URL where the unmodified upstream source for that exact version can be
     downloaded (e.g. `https://ffmpeg.org/releases/ffmpeg-<ver>.tar.xz`,
     `https://github.com/gpac/gpac/releases/tag/v<ver>`,
     `https://github.com/get-iplayer/get_iplayer/releases/tag/v<ver>`).
   - A fall-back contact (the MeedyaDL Issues tracker is fine) where users
     can request a physical copy of the source if the URL ever becomes
     unreachable.
3. The offer must be valid for at least three years from the date of the
   binary distribution. LGPL/GPL standard.

For LGPL specifically, also ensure each LGPL component is shipped as a
**separate executable that the user can substitute** — subprocess invocation
(which we already use for FFmpeg / MP4Box / get_iplayer) satisfies this.

## Reminders for future work

- When wiring up any new bundled component (engine or tool), check its
  licence **first**. If it is LGPL / GPL / MPL / EPL or any other copyleft
  variant, the obligations above kick in **at the bundling step**, not at
  the docs step.
- The right place to harvest upstream `LICENSE` files and emit the
  `OFFER_FOR_SOURCE.txt` is `release.yml` Step 8.5 — it already knows the
  exact versions it just downloaded and is data-driven from `engines.toml`
  + `tool-versions.toml`, so we cannot forget when a new component lands.
- MIT, BSD, ISC, Unlicense, Apache-2.0 → notice only.
- LGPL → notice + source-offer + user-substitutable binary.
- GPL → notice + complete corresponding source (ship the tarball, or
  written offer).
- AGPL → would add "network use is distribution" — we have none today; if
  we ever add one, flag it for legal review before bundling.

## Per-PR enforcement (#806)

The `Licences` GitHub Actions workflow
(`.github/workflows/licences.yml`) runs on every pull request to `main`
and gates merges on three checks:

1. **ACKNOWLEDGEMENTS.md drift** — `npm run check:acknowledgements`.
   Every direct dep in `Cargo.toml` / `package.json` must be named in
   the inventory. Catches the "added a dep, forgot to update docs"
   failure mode.

2. **Upstream licence string drift** — `npm run check:upstream-licences`.
   Reads each direct Rust crate's licence from `cargo metadata` and
   each direct npm runtime dep's licence from
   `node_modules/<pkg>/package.json::license`; compares against
   `ACKNOWLEDGEMENTS.md`. Catches the "upstream re-licensed between
   MeedyaDL releases" failure mode (most commonly MIT → dual
   MIT/Apache-2.0; rarely the permissive → copyleft flip that would
   be a serious compliance risk).

3. **cargo-deny licence allowlist** — `deny.toml` constrains the
   project to permissive-only licences; cargo-deny enforces this
   against the full transitive tree.

The upstream-string check uses an SPDX-aware normaliser:
`LGPL-3.0+` ≡ `LGPL-3.0-or-later`, `MIT/Apache-2.0` ≡
`MIT OR Apache-2.0`, etc. Whitespace / casing / separator
differences are advisory only (not blocking). A genuine licence
change (e.g. our `lucide-react` MIT entry vs upstream's actual ISC,
which the script caught on first run) is a hard fail.

Run all three checks locally via `npm run check:legal`.

## Two files disagreed, and the wrong one was the summary people read (found + fixed 2026-09)

A September 2026 sweep found that **`ACKNOWLEDGEMENTS.md` declared
mp4decrypt / Bento4 as MIT**. It is not — Bento4 is **GPL-2.0 with a
linking exception**, genuinely copyleft, and mp4decrypt is one of the
tools the offline installer bundles onto a user's machine. This repo's own
`THIRD_PARTY_LICENSES.md` already had the licence right. So two files in
the same repository disagreed with each other about the licence of the
same component, and the one most people would actually read first —
`ACKNOWLEDGEMENTS.md`, the short summary table — was the one that was
wrong. Fixed: `ACKNOWLEDGEMENTS.md` now states GPL-2.0-with-linking-
exception and points at `THIRD_PARTY_LICENSES.md#mp4decrypt--bento4` for
the full text.

The same sweep found a second, related problem: the project's documentation
used to describe shipping **"LGPL-only builds of FFmpeg"**. That claim was
never actually supportable. The FFmpeg builds this project's own downloader
fetches for Linux and Windows are pulled from the BtbN/FFmpeg-Builds
project's `-gpl` release assets — the file names say so directly
(`ffmpeg-master-latest-linux64-gpl.tar.xz`,
`ffmpeg-master-latest-win64-gpl.zip`; see `get_ffmpeg_url()` in
`src-tauri/src/services/dependency_manager.rs`) — and nothing in this
project builds FFmpeg itself or checks which compile flags a given build
used, so there was never an "LGPL-only" build in this pipeline for that
claim to point to. `THIRD_PARTY_LICENSES.md` now says plainly: treat every
copy MeedyaDL provides, including the one bundled into the offline
installer, as GPL — not LGPL-only — and offers the FFmpeg path setting as
the way for a user who specifically wants an LGPL-only build to substitute
their own.

Third: the three-year **written offer for source** this file describes
above used to point a reader at "the exact version shipped" without the
release workflow ever actually recording what that version was — the
promise referenced a version record that did not exist. `release.yml`'s
offline-bundle step now writes the real, actually-downloaded version of
each bundled tool into `manifest.json` as it goes (`TOOL_VERSIONS_TSV` in
the "Pre-bundle engines" step), and the generated `OFFER_FOR_SOURCE.txt`
reads the real versions back out of that manifest instead of naming the
tool with no version at all. All three of these were found and fixed in
the same session; if you are reading an older description of this bundle
elsewhere in the repo that still says MIT for Bento4 or "LGPL-only
FFmpeg", that text is the thing that's now stale, not this file.

## Related

- Tracked in #802 (bundling) + #806 (per-PR enforcement).
- [[project-never-worked-pattern]], [[project-comment-accuracy-hazard]] — the
  general pattern this "two files disagreed, the wrong one got read" defect
  belongs to.
- Audit reference: `ACKNOWLEDGEMENTS.md` (component table),
  `THIRD_PARTY_LICENSES.md` (verbatim notices + source offers),
  `src/components/help/HelpViewer.tsx` (in-app About → Open Source
  Acknowledgements), `.github/workflows/release.yml:384-520` (offline-bundle
  pipeline), `.github/workflows/release.yml::Step 8.6` (licence harvest +
  source-offer emission), `.github/workflows/licences.yml` (per-PR
  enforcement), `scripts/check-acknowledgements.mjs` (drift check),
  `scripts/check-upstream-licences.mjs` (string-drift check),
  `src-tauri/src/commands/legal.rs` (embedded-file IPC),
  `src-tauri/tauri.conf.json:46-48` (bundle resources).
