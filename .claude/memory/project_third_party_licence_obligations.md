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

## Related

- Tracked in #802.
- Audit reference: `ACKNOWLEDGEMENTS.md` (component table),
  `src/components/help/HelpViewer.tsx` (in-app About → Open Source
  Acknowledgements), `.github/workflows/release.yml:384-520` (offline-bundle
  pipeline), `src-tauri/tauri.conf.json:46-48` (bundle resources).
