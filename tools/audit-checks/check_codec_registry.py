#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Codec registry cross-source consistency check.

`src-tauri/codecs.toml` is the universal codec registry — compiled into the
binary via `include_str!()` and parsed at runtime by
`models/codec_registry.rs`. It cross-references itself (meta codecs resolve
to concrete codec IDs) and the Rust `SongCodec` enum in
`models/gamdl_options.rs` (each concrete codec's `services.gamdl` value is a
GAMDL CLI string that must be a real `SongCodec` variant). Neither link is
checked by the compiler, so drift is silent until a download fails.

Two checks (both pure cross-source reference validation — the MeedyaDL
analog of WebMS-Intra's `check_sql_columns.py` / `check_route_targets.py`):

  1. META RESOLUTION — every `resolves_to = { <svc> = "<id>" }` target must
     be a concrete codec section that exists in codecs.toml. Catches a meta
     codec left pointing at a renamed/removed concrete codec.

  2. GAMDL CLI VALIDITY — every concrete `[audio.<id>.services]` `gamdl`
     value must be a kebab-case `SongCodec` enum variant. Catches a typo'd
     flag or a `SongCodec` rename that didn't propagate to the registry.

Video/lyrics `gamdl` values are intentionally NOT validated here: GAMDL's
video-codec / lyrics CLI strings have no single canonical Rust enum to check
against, and guessing would produce false positives.

The TOML is parsed with targeted regex (no `tomllib`/`tomli` dependency) so
the script runs on any Python 3 without a venv — matching the audit-checks
house style.

Exit code:
  0 — no findings, OR findings without --strict
  1 — at least one finding AND --strict was passed

Usage:
  python3 tools/audit-checks/check_codec_registry.py [--strict]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CODECS_TOML = REPO_ROOT / "src-tauri" / "codecs.toml"
GAMDL_OPTIONS_RS = REPO_ROOT / "src-tauri" / "src" / "models" / "gamdl_options.rs"

# A concrete codec section header, e.g. `[audio.eac3-atmos]` (exactly two
# dot-segments — NOT the `[audio.x.services]` sub-table).
CONCRETE_SECTION_RE = re.compile(r"^\[(audio|video)\.([a-z0-9-]+)\]\s*$")
# A `[audio.<id>.services]` sub-table header.
SERVICES_SECTION_RE = re.compile(r"^\[(audio|video)\.([a-z0-9-]+)\.services\]\s*$")
# Any section header (used to know when a sub-table block ends).
ANY_SECTION_RE = re.compile(r"^\[")
# A `gamdl = "value"` line.
GAMDL_KV_RE = re.compile(r"""^\s*gamdl\s*=\s*['"]([a-z0-9-]+)['"]""")
# A `resolves_to = { svc = "id", ... }` inline table.
RESOLVES_RE = re.compile(r"resolves_to\s*=\s*\{([^}]*)\}")
INLINE_PAIR_RE = re.compile(r"""([a-z_][a-z0-9_]*)\s*=\s*['"]([a-z0-9-]+)['"]""")


def variant_to_kebab(variant: str) -> str:
    """Convert a Rust enum variant name to its #[serde(rename_all =
    "kebab-case")] string. `AacHeBinaural` -> `aac-he-binaural`."""
    out: list[str] = []
    for i, ch in enumerate(variant):
        if ch.isupper() and i > 0:
            out.append("-")
        out.append(ch.lower())
    return "".join(out)


def collect_song_codec_cli_values() -> set[str]:
    """Derive the set of valid GAMDL song-codec CLI strings from the
    `SongCodec` enum. The enum carries `#[serde(rename_all = "kebab-case")]`;
    any per-variant `#[serde(rename = "x")]` override takes precedence."""
    text = GAMDL_OPTIONS_RS.read_text(encoding="utf-8", errors="ignore")
    m = re.search(r"pub\s+enum\s+SongCodec\s*\{(.*?)\n\}", text, re.DOTALL)
    if not m:
        print("WARNING: SongCodec enum not found in gamdl_options.rs", file=sys.stderr)
        return set()
    body = m.group(1)
    values: set[str] = set()
    pending_rename: str | None = None
    for line in body.splitlines():
        stripped = line.strip()
        if stripped.startswith("//"):
            continue
        rn = re.search(r"""#\[serde\(rename\s*=\s*['"]([^'"]+)['"]""", stripped)
        if rn:
            pending_rename = rn.group(1)
            continue
        vm = re.match(r"([A-Z][A-Za-z0-9]*)\s*,", stripped)
        if vm:
            if pending_rename is not None:
                values.add(pending_rename)
                pending_rename = None
            else:
                values.add(variant_to_kebab(vm.group(1)))
    return values


def parse_codecs_toml() -> tuple[set[str], dict[str, str], list[tuple[int, str, str]]]:
    """Parse codecs.toml with targeted regex.

    Returns:
      concrete_ids   — set of concrete codec IDs (the two-segment sections)
      gamdl_by_id    — {codec_id: gamdl_cli_value} for concrete codecs that
                       declare a [.services] gamdl mapping
      resolves       — list of (line_no, service, target_id) from every
                       resolves_to inline table
    """
    lines = CODECS_TOML.read_text(encoding="utf-8", errors="ignore").splitlines()
    concrete_ids: set[str] = set()
    gamdl_by_id: dict[str, str] = {}
    resolves: list[tuple[int, str, str]] = []

    current_services_id: str | None = None
    for i, line in enumerate(lines):
        cm = CONCRETE_SECTION_RE.match(line)
        if cm:
            concrete_ids.add(cm.group(2))
            current_services_id = None
            continue
        sm = SERVICES_SECTION_RE.match(line)
        if sm:
            current_services_id = sm.group(2)
            continue
        if ANY_SECTION_RE.match(line):
            # Some other section header — leaving any services block.
            current_services_id = None
        if current_services_id is not None:
            gm = GAMDL_KV_RE.match(line)
            if gm:
                gamdl_by_id[current_services_id] = gm.group(1)
        rm = RESOLVES_RE.search(line)
        if rm:
            for pair in INLINE_PAIR_RE.finditer(rm.group(1)):
                resolves.append((i + 1, pair.group(1), pair.group(2)))

    return concrete_ids, gamdl_by_id, resolves


def check() -> int:
    if not CODECS_TOML.exists():
        print(f"WARNING: {CODECS_TOML} not found — skipping", file=sys.stderr)
        return 0

    concrete_ids, gamdl_by_id, resolves = parse_codecs_toml()
    song_cli = collect_song_codec_cli_values()

    print(f"Concrete codec sections          : {len(concrete_ids)}")
    print(f"Concrete codecs with gamdl flag  : {len(gamdl_by_id)}")
    print(f"Meta resolves_to references      : {len(resolves)}")
    print(f"SongCodec CLI values (from Rust) : {len(song_cli)}")
    print()

    findings = 0

    # 1) Meta resolution integrity.
    dangling = [(ln, svc, tgt) for (ln, svc, tgt) in resolves if tgt not in concrete_ids]
    if dangling:
        print("### Meta codec resolves_to a codec ID that does not exist\n")
        for ln, svc, tgt in dangling:
            print(f"  • codecs.toml:{ln} — resolves_to {svc} = \"{tgt}\" but no [audio.{tgt}]/[video.{tgt}] section exists")
        print()
        findings += len(dangling)

    # 2) GAMDL CLI value validity (audio only; see module docstring).
    # We only validate against SongCodec when we successfully parsed it.
    if song_cli:
        bad_gamdl = [
            (cid, val)
            for cid, val in sorted(gamdl_by_id.items())
            # Restrict to AUDIO codecs — video gamdl values (h264/h265) and
            # lyrics values (lrc/srt/ttml) are not SongCodec variants.
            if cid in concrete_ids and val not in song_cli and _is_audio(cid)
        ]
        if bad_gamdl:
            print("### Audio codec gamdl flag is not a known SongCodec CLI value\n")
            for cid, val in bad_gamdl:
                print(f"  • codecs.toml [audio.{cid}.services] — gamdl = \"{val}\" is not a SongCodec variant (kebab-case)")
            print()
            findings += len(bad_gamdl)

    if findings == 0:
        print("OK — codecs.toml meta references resolve and audio gamdl flags match SongCodec.")

    if findings and "--strict" in sys.argv:
        return 1
    return 0


# codecs.toml does not tag concrete sections audio-vs-video in a way the
# regex parser retains per-id, so recover it by re-reading the header set.
_AUDIO_IDS: set[str] | None = None


def _is_audio(codec_id: str) -> bool:
    global _AUDIO_IDS
    if _AUDIO_IDS is None:
        _AUDIO_IDS = set()
        for line in CODECS_TOML.read_text(encoding="utf-8", errors="ignore").splitlines():
            m = re.match(r"^\[audio\.([a-z0-9-]+)\]\s*$", line)
            if m:
                _AUDIO_IDS.add(m.group(1))
    return codec_id in _AUDIO_IDS


if __name__ == "__main__":
    sys.exit(check())
