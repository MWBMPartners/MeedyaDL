#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Codec registry cross-source consistency check.

`src-tauri/codecs.toml` is the universal codec registry — compiled into the
binary via `include_str!()` and parsed at runtime by
`models/codec_registry.rs`. It cross-references itself (meta codecs resolve
to concrete codec IDs) and two Rust enums in `models/gamdl_options.rs`: each
concrete AUDIO codec's `services.gamdl` value must be a real `SongCodec`
variant, and each concrete VIDEO codec's `services.gamdl` value must be a
real `VideoCodec` variant. Neither link is checked by the compiler, so drift
is silent until a download fails.

Three checks (all pure cross-source reference validation — the MeedyaDL
analog of WebMS-Intra's `check_sql_columns.py` / `check_route_targets.py`):

  1. META RESOLUTION — every `resolves_to = { <svc> = "<id>" }` target must
     be a concrete codec section that exists in codecs.toml. Catches a meta
     codec left pointing at a renamed/removed concrete codec.

  2. AUDIO GAMDL CLI VALIDITY — every concrete `[audio.<id>.services]`
     `gamdl` value must be a kebab-case `SongCodec` enum variant (the enum
     carries `#[serde(rename_all = "kebab-case")]`). Catches a typo'd flag
     or a `SongCodec` rename that didn't propagate to the registry.

  3. VIDEO GAMDL CLI VALIDITY — every concrete `[video.<id>.services]`
     `gamdl` value must be a `VideoCodec` enum variant. `VideoCodec` carries
     `#[serde(rename_all = "lowercase")]` — NOT kebab-case like `SongCodec` —
     so `H265`/`H264` serialise to `h265`/`h264`, no inserted dashes. Each
     enum's own `#[serde(rename_all = ...)]` attribute is read to pick the
     right transform, rather than assuming one mode for every enum in this
     file.

Lyrics `gamdl` values are intentionally NOT validated here: GAMDL's lyrics
CLI strings (`lrc`/`srt`/`ttml`/...) have no single canonical Rust enum to
check against, and guessing would produce false positives.

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
# The `#[serde(rename_all = "...")]` attribute immediately above a
# `pub enum <Name> {` declaration — used to pick the right name -> CLI
# string transform for that specific enum (see `collect_enum_cli_values`).
ENUM_RENAME_ALL_RE_TEMPLATE = (
    r"""#\[serde\(rename_all\s*=\s*['"]([a-zA-Z_-]+)['"]\)\]\s*\n"""
    r"""pub\s+enum\s+{enum_name}\s*\{{(.*?)\n\}}"""
)


def variant_to_kebab(variant: str) -> str:
    """Convert a Rust enum variant name to its #[serde(rename_all =
    "kebab-case")] string. `AacHeBinaural` -> `aac-he-binaural`."""
    out: list[str] = []
    for i, ch in enumerate(variant):
        if ch.isupper() and i > 0:
            out.append("-")
        out.append(ch.lower())
    return "".join(out)


def _variant_to_rename_all(variant: str, mode: str) -> str:
    """Apply one `#[serde(rename_all = "<mode>")]` transform to a Rust enum
    variant name. Only the two modes actually used by the enums this script
    reads (`SongCodec`: kebab-case, `VideoCodec`: lowercase) are handled;
    an unrecognised mode falls back to kebab-case; kebab-case is what every
    other enum in gamdl_options.rs used before VideoCodec introduced
    lowercase, so it is the safer default rather than silently returning an
    unmodified variant name that would never match anything."""
    if mode == "lowercase":
        return variant.lower()
    return variant_to_kebab(variant)


def collect_enum_cli_values(enum_name: str) -> set[str]:
    """Derive the set of valid GAMDL CLI strings for one enum in
    `gamdl_options.rs` (`SongCodec`, `VideoCodec`, ...), by reading that
    enum's own `#[serde(rename_all = "...")]` attribute rather than
    assuming every enum in the file uses the same one — `SongCodec` is
    kebab-case, `VideoCodec` is lowercase with no dashes, and a script
    that assumed kebab-case for both would report every real `VideoCodec`
    CLI value (`h265`, `h264`) as invalid, since kebab-casing `H265` byte
    for byte happens to also produce `h265` (no uppercase letter follows
    the first), but that is a coincidence of this exact enum, not
    something to rely on for a differently-named future variant. Any
    per-variant `#[serde(rename = "x")]` override takes precedence over
    the container-level transform, same as SongCodec's."""
    text = GAMDL_OPTIONS_RS.read_text(encoding="utf-8", errors="ignore")
    pattern = ENUM_RENAME_ALL_RE_TEMPLATE.format(enum_name=re.escape(enum_name))
    m = re.search(pattern, text, re.DOTALL)
    if not m:
        print(f"WARNING: {enum_name} enum (with its rename_all attribute) not found in gamdl_options.rs", file=sys.stderr)
        return set()
    mode, body = m.group(1), m.group(2)
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
                values.add(_variant_to_rename_all(vm.group(1), mode))
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
    song_cli = collect_enum_cli_values("SongCodec")
    video_cli = collect_enum_cli_values("VideoCodec")

    print(f"Concrete codec sections          : {len(concrete_ids)}")
    print(f"Concrete codecs with gamdl flag  : {len(gamdl_by_id)}")
    print(f"Meta resolves_to references      : {len(resolves)}")
    print(f"SongCodec CLI values (from Rust) : {len(song_cli)}")
    print(f"VideoCodec CLI values (from Rust): {len(video_cli)}")
    print()

    findings = 0

    # 0) Can this script actually read what it's about to check against?
    #
    # `collect_enum_cli_values()` comes back empty in two situations that
    # both mean the same thing: it could not read the enum, not that the
    # enum has nothing in it. SongCodec and VideoCodec always have several
    # variants, so an empty result means either the enum's `pub enum Name {`
    # line couldn't be found at all, or its `#[serde(rename_all = "...")]`
    # attribute isn't sitting directly above it any more -- for example
    # because someone inserted another attribute (`#[allow(...)]`, a second
    # derive, a doc comment) in between, which is exactly the kind of small,
    # well-meaning edit nobody would expect to break an unrelated Python
    # script.
    #
    # The old code treated that silence as "nothing to check" and skipped
    # checks 2/3 below without a word -- so a real typo in codecs.toml's
    # `gamdl` value (say, a misspelt codec name) would sail through with
    # this script still printing "OK". An audit that goes quiet exactly
    # when it stops being able to see is worse than no audit: it looks
    # green while checking nothing. So this is reported as a finding of
    # its own, and checks 2/3 are skipped only because there is nothing
    # correct to compare against -- not silently, and not while still
    # claiming a clean pass.
    if not song_cli:
        print("### Could not read SongCodec's CLI values from gamdl_options.rs\n")
        print(
            f"  • {GAMDL_OPTIONS_RS.relative_to(REPO_ROOT).as_posix()} — SongCodec's "
            f"#[serde(rename_all = ...)] attribute and variant list could not be "
            f"parsed (it may no longer sit directly above `pub enum SongCodec {{`). "
            f"Audio codec gamdl flags in codecs.toml were NOT checked against it -- "
            f"this is a failure to check, not a clean result."
        )
        print()
        findings += 1
    if not video_cli:
        print("### Could not read VideoCodec's CLI values from gamdl_options.rs\n")
        print(
            f"  • {GAMDL_OPTIONS_RS.relative_to(REPO_ROOT).as_posix()} — VideoCodec's "
            f"#[serde(rename_all = ...)] attribute and variant list could not be "
            f"parsed (it may no longer sit directly above `pub enum VideoCodec {{`). "
            f"Video codec gamdl flags in codecs.toml were NOT checked against it -- "
            f"this is a failure to check, not a clean result."
        )
        print()
        findings += 1

    # 1) Meta resolution integrity.
    dangling = [(ln, svc, tgt) for (ln, svc, tgt) in resolves if tgt not in concrete_ids]
    if dangling:
        print("### Meta codec resolves_to a codec ID that does not exist\n")
        for ln, svc, tgt in dangling:
            print(f"  • codecs.toml:{ln} — resolves_to {svc} = \"{tgt}\" but no [audio.{tgt}]/[video.{tgt}] section exists")
        print()
        findings += len(dangling)

    # 2) Audio GAMDL CLI value validity. We only validate against SongCodec
    # when we successfully parsed it.
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

    # 3) Video GAMDL CLI value validity — same shape as (2), but against
    # `VideoCodec`, which is lowercase-with-no-dashes, not kebab-case (see
    # `collect_enum_cli_values`'s docstring for why that distinction
    # matters to get right rather than assumed).
    if video_cli:
        bad_video_gamdl = [
            (cid, val)
            for cid, val in sorted(gamdl_by_id.items())
            if cid in concrete_ids and val not in video_cli and _is_video(cid)
        ]
        if bad_video_gamdl:
            print("### Video codec gamdl flag is not a known VideoCodec CLI value\n")
            for cid, val in bad_video_gamdl:
                print(f"  • codecs.toml [video.{cid}.services] — gamdl = \"{val}\" is not a VideoCodec variant (lowercase, no dashes)")
            print()
            findings += len(bad_video_gamdl)

    if findings == 0:
        print(
            "OK — codecs.toml meta references resolve, audio gamdl flags match "
            "SongCodec, and video gamdl flags match VideoCodec."
        )

    if findings and "--strict" in sys.argv:
        return 1
    return 0


# codecs.toml does not tag concrete sections audio-vs-video in a way the
# regex parser retains per-id, so recover it by re-reading the header set.
# Cached once per process since CODECS_TOML doesn't change mid-run.
_AUDIO_IDS: set[str] | None = None
_VIDEO_IDS: set[str] | None = None


def _codec_section_ids(kind: str) -> set[str]:
    """The set of concrete codec IDs under `[<kind>.<id>]` headers, where
    `kind` is `"audio"` or `"video"`."""
    ids: set[str] = set()
    pattern = re.compile(rf"^\[{kind}\.([a-z0-9-]+)\]\s*$")
    for line in CODECS_TOML.read_text(encoding="utf-8", errors="ignore").splitlines():
        m = pattern.match(line)
        if m:
            ids.add(m.group(1))
    return ids


def _is_audio(codec_id: str) -> bool:
    global _AUDIO_IDS
    if _AUDIO_IDS is None:
        _AUDIO_IDS = _codec_section_ids("audio")
    return codec_id in _AUDIO_IDS


def _is_video(codec_id: str) -> bool:
    global _VIDEO_IDS
    if _VIDEO_IDS is None:
        _VIDEO_IDS = _codec_section_ids("video")
    return codec_id in _VIDEO_IDS


if __name__ == "__main__":
    sys.exit(check())
