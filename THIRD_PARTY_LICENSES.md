# Third-Party Licences

This file contains the **verbatim** upstream copyright notices and licence text
for the external engines, tools, and runtimes that MeedyaDL invokes or bundles.
It is shipped with the application alongside `ACKNOWLEDGEMENTS.md` (which lists
inventory + categories) and `LICENSE` (which covers MeedyaDL's own code).

If you obtained MeedyaDL in **offline-installer form** (the `~300 MB` artefact
produced by the `bundle_engines=true` release path) — the binaries listed under
"External Tools" below are physically inside your installer and the notices in
this file are part of their redistribution. The "Source Offers" appendix at the
end of this file constitutes the MeedyaDL project's written offer for the
complete corresponding source of every LGPL/GPL-licensed component shipped
inside the offline installer.

If you obtained MeedyaDL in **tiny-installer form** — the engines and tools
list themselves from PyPI / GitHub Releases onto your machine at first launch;
the notices below still apply to those copies even though MeedyaDL itself
didn't redistribute them.

Component inventory (which engines / tools / Rust crates / npm packages
MeedyaDL depends on, with versions and a one-line purpose) lives in
[ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md). This file holds the legal text.

---

## Table of Contents

### Download engines

- [GAMDL](#gamdl) — MIT
- [votify](#votify) — MIT (planned, M9)
- [yt-dlp](#yt-dlp) — Unlicense (planned, M10)
- [get_iplayer](#get_iplayer) — GPL-3.0 (planned, M8)

### External tools (binaries we invoke)

- [FFmpeg](#ffmpeg) — LGPL-2.1-or-later (some builds GPL — see notice)
- [mp4decrypt / Bento4](#mp4decrypt--bento4) — GPL-2.0 with linking exception (the Bento4 SDK that ships mp4decrypt)
- [N_m3u8DL-RE](#n_m3u8dl-re) — MIT
- [MP4Box / GPAC](#mp4box--gpac) — LGPL-2.1
- [MediaInfo](#mediainfo) — BSD-2-Clause
- [Python](#python) — PSF (Python Software Foundation Licence)
- [rclone](#rclone) — MIT (optional, installed on-demand for Cloud Destinations)

### Source offers (LGPL / GPL components)

- [Written Offer for Source — FFmpeg / MP4Box / get_iplayer / mp4decrypt](#source-offers)

### Standard licence texts

- [MIT Licence](#mit-licence-standard-text)
- [BSD 2-Clause "Simplified" Licence](#bsd-2-clause-simplified-licence-standard-text)
- [Unlicense](#unlicense-standard-text)

---

## Download Engines

### GAMDL

Apple Music download engine. Repository: <https://github.com/glomatico/gamdl>.

```text
MIT License

Copyright (c) 2024 Glomatico

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Upstream LICENSE file: <https://github.com/glomatico/gamdl/blob/main/LICENSE>.

### votify

Spotify download engine (planned for M9 / v2.1.0).
Repository: <https://github.com/glomatico/votify>.
Licensed under the **MIT License**. See the
[Standard MIT Licence text](#mit-licence-standard-text) below; the copyright
line for votify is *"Copyright (c) Glomatico"*. Upstream LICENSE file:
<https://github.com/glomatico/votify/blob/main/LICENSE>.

### yt-dlp

YouTube / BBC iPlayer fallback download engine (planned for M8 / M10).
Repository: <https://github.com/yt-dlp/yt-dlp>.
Released into the public domain under the
[Unlicense](#unlicense-standard-text). Upstream LICENSE file:
<https://github.com/yt-dlp/yt-dlp/blob/master/LICENSE>.

### get_iplayer

BBC iPlayer download engine (planned for M8 / v2.0.0). Repository:
<https://github.com/get-iplayer/get_iplayer>.

```text
This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
```

Upstream LICENSE file (full GPL-3.0 text):
<https://github.com/get-iplayer/get_iplayer/blob/master/LICENSE>.
Canonical full GPL-3.0 text: <https://www.gnu.org/licenses/gpl-3.0.txt>.

**MeedyaDL's relationship to get_iplayer:** MeedyaDL invokes get_iplayer
as a **subprocess** (separate executable), never linking it as a library.
This means MeedyaDL's own MIT-licensed code is protected from GPL
propagation by the GPL's "mere aggregation" exception. The GPL
applies to the get_iplayer binary itself, not to MeedyaDL.

If you obtained the offline-installer form of MeedyaDL that includes
get_iplayer, see [Source Offers](#source-offers) below for the
corresponding-source-code offer.

---

## External Tools

### FFmpeg

Audio / video processing, remuxing, ReplayGain analysis. Project home:
<https://ffmpeg.org/>.

```text
Most files in FFmpeg are under the GNU Lesser General Public License version
2.1 or later (LGPL v2.1+). Read the file COPYING.LGPLv2.1 for details. Some
other files have MIT/X11/BSD-style licenses. In combination the LGPL v2.1+
applies to FFmpeg.

Some optional parts of FFmpeg are licensed under the GNU General Public
License version 2 or later (GPL v2+). See the file COPYING.GPLv2 for
details. None of these parts are used by default; you have to explicitly
opt in to use them. If any of the GPL-incompatible options are used, the
combined work becomes GPL-licensed.

This software is provided "as is", without warranty of any kind, express or
implied. In no event shall the authors or copyright holders be liable for
any claim, damages or other liability, whether in an action of contract,
tort or otherwise, arising from, out of or in connection with the software
or the use or other dealings in the software.
```

Upstream LICENSE.md (canonical): <https://github.com/FFmpeg/FFmpeg/blob/master/LICENSE.md>.
Full LGPL-2.1 text: <https://www.gnu.org/licenses/old-licenses/lgpl-2.1.txt>.

**MeedyaDL ships LGPL-only builds of FFmpeg** in the offline-installer
artefact — i.e. without `--enable-gpl`-gated codecs. The user can
substitute their own FFmpeg build via `Settings → Tools → FFmpeg path`,
which satisfies the LGPL's "user must be able to substitute their own
build" requirement.

If you obtained the offline-installer form of MeedyaDL, see
[Source Offers](#source-offers) below for the offer of complete
corresponding source for the LGPL portions of FFmpeg.

### mp4decrypt / Bento4

MP4 DRM decryption utility from the Bento4 SDK. Project home:
<https://www.bento4.com/>. Source:
<https://github.com/axiomatic-systems/Bento4>.

```text
Bento4 is licensed under the GNU General Public License Version 2,
with the following clarification and special exception:

Linking Bento4 statically or dynamically with other modules is making a
combined work based on Bento4. Thus, the terms and conditions of the GNU
General Public License cover the whole combination.

As a special exception, the copyright holders of Bento4 give you permission
to combine Bento4 program with free software programs or libraries that are
released under the GNU LGPL.

If you wish to use Bento4 in a commercial product without the terms of the
GPL applying to your product, you can purchase a commercial licence from
Axiomatic Systems.

Copyright (C) 2002-2024 Axiomatic Systems, LLC.
```

Upstream LICENSE.txt:
<https://github.com/axiomatic-systems/Bento4/blob/master/Documents/LICENSE.txt>.

**MeedyaDL's relationship to mp4decrypt:** the binary is invoked as a
subprocess, never linked. The "linking exception" therefore does not
apply to MeedyaDL — only to other binaries that link with Bento4.

### N_m3u8DL-RE

HLS/DASH stream downloader. Repository:
<https://github.com/nilaoda/N_m3u8DL-RE>.
Licensed under the **MIT License**, *Copyright (c) 2022 nilaoda*. See the
[Standard MIT Licence text](#mit-licence-standard-text). Upstream LICENSE
file: <https://github.com/nilaoda/N_m3u8DL-RE/blob/main/LICENSE>.

### MP4Box / GPAC

Media container toolkit. Project home: <https://gpac.io/>. Source:
<https://github.com/gpac/gpac>.

```text
GPAC is free software: you can redistribute it and/or modify
it under the terms of the GNU Lesser General Public License as published by
the Free Software Foundation, either version 2.1 of the License, or
(at your option) any later version.

GPAC is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU Lesser General Public License for more details.

You should have received a copy of the GNU Lesser General Public License
along with GPAC. If not, see <https://www.gnu.org/licenses/>.

Copyright (c) Telecom ParisTech 2000-2025
```

Upstream LICENSE file:
<https://github.com/gpac/gpac/blob/master/COPYING>.
Full LGPL-2.1 text: <https://www.gnu.org/licenses/old-licenses/lgpl-2.1.txt>.

**MeedyaDL ships MP4Box / GPAC** in the offline-installer artefact;
the binary is invoked as a subprocess, never linked. The user can
substitute their own MP4Box build via `Settings → Tools → MP4Box path`,
which satisfies the LGPL's user-substitution requirement.

If you obtained the offline-installer form of MeedyaDL, see
[Source Offers](#source-offers) below for the offer of complete
corresponding source.

### MediaInfo

Media file analysis and codec detection. Project home:
<https://mediaarea.net/en/MediaInfo>. Source:
<https://github.com/MediaArea/MediaInfo>.

```text
Copyright (c) 2002-2025, MediaArea.net SARL
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice,
   this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
POSSIBILITY OF SUCH DAMAGE.
```

Upstream LICENSE file:
<https://github.com/MediaArea/MediaInfo/blob/master/License.html>.

### Python

Runtime for pip-based download engines (GAMDL, planned votify / yt-dlp).
Project home: <https://www.python.org/>. The CPython runtime is licensed
under the **PSF License Agreement**.

```text
PSF LICENSE AGREEMENT FOR PYTHON

1. This LICENSE AGREEMENT is between the Python Software Foundation
("PSF"), and the Individual or Organization ("Licensee") accessing and
otherwise using this software ("Python") in source or binary form and
its associated documentation.

2. Subject to the terms and conditions of this License Agreement, PSF
hereby grants Licensee a nonexclusive, royalty-free, world-wide license
to reproduce, analyze, test, perform and/or display publicly, prepare
derivative works, distribute, and otherwise use Python alone or in any
derivative version, provided, however, that PSF's License Agreement and
PSF's notice of copyright, i.e., "Copyright © 2001-2025 Python Software
Foundation; All Rights Reserved" are retained in Python alone or in any
derivative version prepared by Licensee.

3. In the event Licensee prepares a derivative work that is based on or
incorporates Python or any part thereof, and wants to make the
derivative work available to others as provided herein, then Licensee
hereby agrees to include in any such work a brief summary of the
changes made to Python.

4. PSF is making Python available to Licensee on an "AS IS" basis. PSF
MAKES NO REPRESENTATIONS OR WARRANTIES, EXPRESS OR IMPLIED. BY WAY OF
EXAMPLE, BUT NOT LIMITATION, PSF MAKES NO AND DISCLAIMS ANY REPRESENTATION
OR WARRANTY OF MERCHANTABILITY OR FITNESS FOR ANY PARTICULAR PURPOSE OR
THAT THE USE OF PYTHON WILL NOT INFRINGE ANY THIRD PARTY RIGHTS.

5. PSF SHALL NOT BE LIABLE TO LICENSEE OR ANY OTHER USERS OF PYTHON FOR
ANY INCIDENTAL, SPECIAL, OR CONSEQUENTIAL DAMAGES OR LOSS AS A RESULT OF
MODIFYING, DISTRIBUTING, OR OTHERWISE USING PYTHON, OR ANY DERIVATIVE
THEREOF, EVEN IF ADVISED OF THE POSSIBILITY THEREOF.

6. This License Agreement will automatically terminate upon a material
breach of its terms and conditions.

7. Nothing in this License Agreement shall be deemed to create any
relationship of agency, partnership, or joint venture between PSF and
Licensee. This License Agreement does not grant permission to use PSF
trademarks or trade name in a trademark sense to endorse or promote
products or services of Licensee, or any third party.

8. By copying, installing or otherwise using Python, Licensee agrees to
be bound by the terms and conditions of this License Agreement.
```

MeedyaDL bundles the
[python-build-standalone](https://github.com/astral-sh/python-build-standalone)
distribution of CPython under the offline-installer path. Its own licence
is MPL-2.0 with the embedded Python licensed under the PSF Agreement above.

### rclone

Cloud-storage transport for direct-to-cloud downloads (Google Drive,
Dropbox, OneDrive, S3, …). **Optional dependency** — only installed
when the user enables a Cloud Destination in Settings; the rest of
MeedyaDL functions normally when rclone is absent. Project home:
<https://rclone.org/>. Repository: <https://github.com/rclone/rclone>.

```text
MIT License

Copyright (C) 2012 by Nick Craig-Wood https://www.craig-wood.com/nick/

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
```

MeedyaDL invokes rclone as an unmodified upstream subprocess; no rclone
source is statically linked into MeedyaDL's own binary. Because rclone is
permissively MIT-licensed, no source-offer is required for it. Pre-built
binaries are fetched on-demand from rclone's official GitHub releases
(<https://github.com/rclone/rclone/releases>), with the
`MeedyaSuite/MeedyaDL-Tools` mirror as a fallback.

---

## Source Offers

MeedyaDL hereby makes the following **written offer for complete
corresponding source code**, valid for **three (3) years** from the date
of distribution of each version, for the LGPL- and GPL-licensed components
shipped inside the offline-installer artefact:

> **Source-offer policy.** For each LGPL- or GPL-licensed component
> listed below, the unmodified upstream source for the exact version
> shipped is available from the linked upstream release page. If the
> linked URL ever becomes unreachable, you may obtain a copy by opening
> an issue at <https://github.com/MWBMPartners/MeedyaDL/issues> with the
> subject "Source request — \<component\> \<version\>"; the maintainers
> will respond with either a working URL or a physical copy delivered
> at no charge beyond the cost of physical distribution.
>
> If MeedyaDL ever ships a **modified** build of an LGPL/GPL component,
> the modified source tree will be published in a sibling repository
> under <https://github.com/MWBMPartners/MeedyaDL-Tools> alongside the
> binary; the link above will redirect there for the affected version.
> As of the current release, no LGPL/GPL component is modified.

### FFmpeg

- Upstream source archive: `https://ffmpeg.org/releases/ffmpeg-<VERSION>.tar.xz`
  where `<VERSION>` is the FFmpeg version named in the bundled
  `manifest.json` (or recorded in `bundled-deps/THIRD_PARTY_LICENSES.md`
  when produced by the offline-installer pipeline).
- Source repository: <https://github.com/FFmpeg/FFmpeg>.

### MP4Box / GPAC

- Upstream tagged release: `https://github.com/gpac/gpac/releases/tag/v<VERSION>`
  where `<VERSION>` is the GPAC version named in the bundled `manifest.json`.
- Source repository: <https://github.com/gpac/gpac>.

### mp4decrypt / Bento4

- Upstream source: <https://github.com/axiomatic-systems/Bento4>.
- Tagged releases: <https://github.com/axiomatic-systems/Bento4/releases>.

### get_iplayer (when wired up — M8 / v2.0.0)

- Upstream tagged release:
  `https://github.com/get-iplayer/get_iplayer/releases/tag/v<VERSION>`.
- Source repository: <https://github.com/get-iplayer/get_iplayer>.

---

## Standard Licence Texts

### MIT Licence — standard text

```text
MIT License

Copyright (c) <YEAR> <COPYRIGHT HOLDER>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### BSD 2-Clause "Simplified" Licence — standard text

```text
Copyright (c) <YEAR> <COPYRIGHT HOLDER>
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice,
   this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
POSSIBILITY OF SUCH DAMAGE.
```

### Unlicense — standard text

```text
This is free and unencumbered software released into the public domain.

Anyone is free to copy, modify, publish, use, compile, sell, or distribute
this software, either in source code form or as a compiled binary, for any
purpose, commercial or non-commercial, and by any means.

In jurisdictions that recognize copyright laws, the author or authors of
this software dedicate any and all copyright interest in the software to
the public domain. We make this dedication for the benefit of the public
at large and to the detriment of our heirs and successors. We intend this
dedication to be an overt act of relinquishment in perpetuity of all
present and future rights to this software under copyright law.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

For more information, please refer to <https://unlicense.org/>
```

---

## Rust crates and npm packages

The hundreds of indirect Rust crates (resolved transitively from
[`src-tauri/Cargo.toml`](src-tauri/Cargo.toml)) and npm packages (resolved
transitively from [`package.json`](package.json)) are predominantly under
**permissive licences** (MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause,
ISC). Their individual licence files are preserved inside the published
binary artefacts in compressed form by the platform's standard package
metadata convention:

- **Rust crates**: cargo's standard `.crate` package format preserves the
  `LICENSE` files of each dependency next to its source. When MeedyaDL is
  built, these files are statically linked into the binary but their
  text is preserved in the cargo registry cache, mirrored to
  <https://crates.io>. Every crate listed in
  [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml) and its transitive
  dependency tree has a copy of its licence file at
  <https://crates.io/crates/\<crate\>>.
- **npm packages**: each shipped `node_modules/<pkg>/LICENSE` file is
  preserved in the source-of-truth `package-lock.json`. Every package
  listed in [`package.json`](package.json) and its transitive
  dependency tree has a copy of its licence at
  <https://www.npmjs.com/package/\<pkg\>>.

The [`src-tauri/deny.toml`](src-tauri/deny.toml) file constrains the
project to a permissive-only allowlist; `cargo deny check licenses` is
run in CI on every push to guarantee no Rust dependency with a copyleft
licence sneaks in transitively. The npm side has a similar
manually-audited allowlist documented in [ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md).

If you require the verbatim text of a specific transitive licence not
reproduced above, please open an issue at
<https://github.com/MWBMPartners/MeedyaDL/issues> and the maintainers
will append it to this file.

---

*This file is generated by hand and reviewed at each release. Last
revised: 2026-05-17 (issue #802). The complementary inventory file
[ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md) lists every direct
dependency with versions and a one-line purpose.*
