#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
scripts/smoke-tests/gamdl_live_smoke.py
========================================

Live, human-run verification that a real GAMDL download actually produces a
correct audio file end-to-end. Stdlib-only Python 3.10+ so it runs on
MeedyaDL's own managed interpreter with zero extra dependencies.

WHY THIS EXISTS
---------------
GAMDL 3.8.2 and 3.8.3 shipped a wrapper-decrypt data-corruption bug: the
Rust `_ammuxer` extension queued batched decrypted samples but did not
flush them before an immediate write, so payload order didn't match the
container's sample-size table. The user-visible symptom is a TRUNCATED OR
CORRUPT SONG ENDING on wrapper-decrypted downloads. GAMDL 3.8.4 fixed this
(`flush_then_write_immediate()` in `ammuxer/src/media.rs`). MeedyaDL's
CLAUDE.md and its GAMDL-version audits recommend 3.8.5, but the actual
verification gate — a real download, decoded and inspected end-to-end —
has never been run. This script is that gate. Song-ending integrity
(Phase 4) is the core thing it exists to catch; everything else is
supporting infrastructure to get there reliably and repeatably.

THIS SCRIPT IS DELIBERATELY **NOT** WIRED INTO ANY CI WORKFLOW. It needs a
real Apple Music subscription's cookies and/or a real running wrapper-v2
daemon, plus a human ears-on listen at the end — none of which CI can ever
provide. See scripts/smoke-tests/README.md for the full run book.

WHAT THIS DOES, PHASE BY PHASE
------------------------------
  1. Environment       -- confirms the resolved interpreter has exactly the
                           expected GAMDL version installed, that the CLI
                           runs, and that the compiled `gamdl._ammuxer`
                           extension (where the 3.8.4 fix lives) imports.
  2. Wrapper preflight -- (wrapper/both modes only) TCP-connects to the
                           m3u8 and decrypt sockets, and asserts the
                           wrapper-v2 daemon's `/me` reports EXACTLY
                           version "0.0.2" (GAMDL 3.8.2+ hard-requires
                           this and aborts on any mismatch).
  3. Download matrix   -- runs real GAMDL downloads into a fresh temp
                           directory: non-wrapper leg downloads `aac`
                           (works without wrapper auth on GAMDL 3.8+);
                           wrapper leg downloads `alac` (still wrapper-
                           dependent on 3.8.4) AND `atmos` (exercises the
                           other wrapper-dependent decrypt shape).
  4. Song-ending        -- THE CORE CHECK. For every downloaded file:
     integrity              (a) a full `ffmpeg -f null -` decode produces
                                 no stderr output,
                             (b) the decoded sample count (nb_read_samples
                                 / sample_rate) matches the container's
                                 reported duration within 250ms (AAC
                                 encoder priming/padding tolerance),
                             (c) a `-sseof -5` TAIL decode of just the
                                 final 5 seconds is clean -- this is what
                                 specifically catches the 3.8.2/3.8.3
                                 truncated-ending corruption, since a
                                 corrupt tail can otherwise slip past a
                                 whole-file decode that merely logs a
                                 warning rather than an error,
                             (d) if `--expected-duration-ms` was given,
                                 the container duration is within 1s of it.
  5. Report            -- TAP-style pass/fail to stdout for a human to
                           read, plus a machine-readable JSON report
                           (`--json`) shaped to be pasted straight into a
                           `.github/audits/*.md` file.

Every check is independent and recorded; a failure in an earlier phase
does not stop later phases from at least attempting to run and reporting
their own results (except where a later phase has a hard dependency on an
earlier phase's *output*, e.g. Phase 4 needs Phase 3's downloaded files).
Exit code is non-zero if ANY check failed.

USAGE
-----
    # Non-wrapper only (cookies-based auth), AAC:
    python3 gamdl_live_smoke.py --auto --tools-dir /path/to/tools \\
        --url "https://music.apple.com/us/song/example/1234567890" \\
        --mode nonwrapper --cookies ./cookies.txt --json report.json

    # Wrapper only (ALAC + Atmos), against a locally running wrapper-v2:
    python3 gamdl_live_smoke.py --auto --tools-dir /path/to/tools \\
        --url "https://music.apple.com/us/song/example/1234567890" \\
        --mode wrapper \\
        --wrapper-url http://127.0.0.1:30020 \\
        --wrapper-m3u8 127.0.0.1:20020 \\
        --wrapper-decrypt 127.0.0.1:10020 \\
        --json report.json

    # Both legs, with a known-good duration cross-check:
    python3 gamdl_live_smoke.py --auto --tools-dir /path/to/tools \\
        --url "https://music.apple.com/us/song/example/1234567890" \\
        --mode both --cookies ./cookies.txt \\
        --wrapper-url http://127.0.0.1:30020 \\
        --wrapper-m3u8 127.0.0.1:20020 --wrapper-decrypt 127.0.0.1:10020 \\
        --expected-duration-ms 214000 --json report.json

See scripts/smoke-tests/README.md for the full run book, the platform
matrix, and what genuinely needs a human.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import platform
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

# Bundle identifiers MeedyaDL's app-data directory has used. The "new"
# identifier is current (post the 2026-07-24 rename); "old" is kept as a
# fallback probe target because a machine that hasn't relaunched the app
# since upgrading may still only have the old directory. Mirrors
# src-tauri/src/utils/platform.rs's OLD_BUNDLE_IDENTIFIER / CURRENT_BUNDLE_IDENTIFIER.
BUNDLE_IDENTIFIERS = ("com.meedyasuite.meedyadl", "io.github.meedyadl")

# Audio file extensions GAMDL can produce, used to locate the downloaded
# track inside the (otherwise GAMDL-templated) output directory tree.
AUDIO_EXTENSIONS = {".m4a", ".m4b", ".flac", ".mp3", ".ogg", ".wav"}

# MeedyaDL's own tools-directory naming convention: {tools_dir}/{tool_id}/{binary}.
# Mirrors get_tool_binary_path() in src-tauri/src/services/dependency_manager.rs.
# ffprobe is intentionally absent here -- it lives alongside ffmpeg's binary
# (tools_dir/ffmpeg/ffprobe), not in its own tool_id directory, matching how
# MeedyaDL's dependency manager bundles it (install_companion_ffprobe()).
TOOL_BINARY_NAMES = {
    "ffmpeg": "ffmpeg",
    "mp4decrypt": "mp4decrypt",
    "nm3u8dlre": "N_m3u8DL-RE",
    "mp4box": "MP4Box",
}

# GAMDL's own CLI flags for pointing at each external tool binary.
TOOL_CLI_FLAGS = {
    "ffmpeg": "--ffmpeg-path",
    "mp4decrypt": "--mp4decrypt-path",
    "nm3u8dlre": "--nm3u8dlre-path",
    "mp4box": "--mp4box-path",
}

DEFAULT_GAMDL_SPEC_CEILING = "3.8.5"


# ---------------------------------------------------------------------------
# Result plumbing
# ---------------------------------------------------------------------------


@dataclass
class CheckResult:
    """A single named pass/fail, with optional free-text detail."""

    name: str
    ok: bool
    detail: str = ""
    skipped: bool = False


@dataclass
class Phase:
    """A named group of related checks (one of the five phases above)."""

    name: str
    checks: list[CheckResult] = field(default_factory=list)

    def add(self, name: str, ok: bool, detail: str = "") -> None:
        self.checks.append(CheckResult(name, ok, detail))

    def add_skip(self, name: str, reason: str) -> None:
        """Records a check as skipped. Skipped checks always count as
        'ok' for exit-code purposes (there was nothing to fail), but are
        rendered distinctly in TAP output (# SKIP) and JSON."""
        self.checks.append(CheckResult(name, True, reason, skipped=True))


# ---------------------------------------------------------------------------
# Small helpers
# ---------------------------------------------------------------------------


def run_cmd(
    cmd: list[str], timeout: Optional[float] = None
) -> subprocess.CompletedProcess:
    """Runs a subprocess capturing stdout/stderr as text. Callers handle
    their own exceptions (TimeoutExpired, OSError) -- this is a thin,
    deliberately un-magic wrapper so every call site's error handling is
    visible at the call site rather than hidden in a shared try/except."""
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


def split_host_port(value: str) -> tuple[str, str]:
    """Splits a 'host:port' string at the LAST colon (IPv6-safe), mirroring
    Rust's `addr.rsplit_once(':')` in gamdl_options.rs's wrapper-decrypt-host
    /-port emission. Raises ValueError with a clear message on a malformed
    value rather than silently returning a wrong split."""
    if ":" not in value:
        raise ValueError(f"expected 'host:port', got {value!r} (no colon)")
    host, _, port = value.rpartition(":")
    if not host or not port:
        raise ValueError(f"expected 'host:port', got {value!r}")
    return host, port


def platform_triple() -> str:
    """A short platform identifier suitable for a report / audit doc, e.g.
    'macos-aarch64', 'windows-x86_64', 'linux-aarch64'."""
    system = platform.system()
    machine = platform.machine().lower()
    arch_map = {
        "amd64": "x86_64",
        "x86_64": "x86_64",
        "arm64": "aarch64",
        "aarch64": "aarch64",
        "armv7l": "armv7",
    }
    arch = arch_map.get(machine, machine)
    os_map = {"Darwin": "macos", "Windows": "windows", "Linux": "linux"}
    osname = os_map.get(system, system.lower())
    return f"{osname}-{arch}"


def tcp_check(host: str, port: str, timeout: float = 3.0) -> tuple[bool, str]:
    """A single 3-second TCP connect attempt. Returns (ok, detail)."""
    try:
        with socket.create_connection((host, int(port)), timeout=timeout):
            return True, ""
    except (OSError, ValueError) as exc:
        return False, str(exc)


def find_audio_files(root: Path) -> list[Path]:
    """Recursively locates audio files GAMDL may have produced under an
    output directory (whose exact subfolder layout depends on the user's
    filename/folder templates, so we search rather than assume a shape)."""
    return sorted(
        p for p in root.rglob("*") if p.is_file() and p.suffix.lower() in AUDIO_EXTENSIONS
    )


# ---------------------------------------------------------------------------
# Python interpreter resolution (--python-bin / --auto)
# ---------------------------------------------------------------------------


def _candidate_app_data_roots() -> list[Path]:
    """Enumerates MeedyaDL's app-data root directory across platforms and
    both bundle-identifier eras, newest identifier first. See
    src-tauri/src/utils/platform.rs module doc for the authoritative
    per-OS layout this mirrors:
      - macOS:   ~/Library/Application Support/{identifier}/
      - Windows: %APPDATA%/{identifier}/
      - Linux:   ~/.local/share/{identifier}/ (or $XDG_DATA_HOME)
    """
    system = platform.system()
    bases: list[Path] = []

    if system == "Windows":
        appdata = os.environ.get("APPDATA")
        if appdata:
            bases.append(Path(appdata))
    elif system == "Darwin":
        bases.append(Path.home() / "Library" / "Application Support")
    else:
        xdg = os.environ.get("XDG_DATA_HOME")
        bases.append(Path(xdg) if xdg else Path.home() / ".local" / "share")

    return [base / identifier for base in bases for identifier in BUNDLE_IDENTIFIERS]


def resolve_auto_python() -> Optional[Path]:
    """Probes MeedyaDL's managed app-data directories for a usable Python
    interpreter, handling BOTH on-disk layouts a managed install can have:
      - portable (python-build-standalone): Windows `python.exe` at the
        root of the python/ dir; macOS/Linux `bin/python3`.
      - system-Python venv (#1017, "reuse a compatible system Python"):
        Windows `Scripts/python.exe`; macOS/Linux still `bin/python3`
        (identical to the portable layout on Unix).
    Mirrors platform::resolve_managed_python_binary()'s existence-probing
    logic (portable path checked first, venv path as fallback) -- see that
    function's doc comment for why Windows needs both branches and Unix
    doesn't.
    """
    is_windows = platform.system() == "Windows"
    for root in _candidate_app_data_roots():
        python_dir = root / "python"
        if is_windows:
            candidates = [python_dir / "python.exe", python_dir / "Scripts" / "python.exe"]
        else:
            candidates = [python_dir / "bin" / "python3"]
        for candidate in candidates:
            if candidate.exists():
                return candidate
    return None


# ---------------------------------------------------------------------------
# External tool resolution (--tools-dir / PATH fallback)
# ---------------------------------------------------------------------------


def resolve_tool(tools_dir: Optional[Path], tool_id: str) -> Optional[Path]:
    """Resolves one of MeedyaDL's four required external tools. Tries the
    MeedyaDL-managed layout ({tools_dir}/{tool_id}/{binary_name}[.exe])
    first, then falls back to the system PATH -- matching the app's own
    three-tier resolution philosophy (managed install > system PATH being
    the reverse order here on purpose: --tools-dir is the explicit,
    intentional choice a human operator made when invoking this script, so
    it wins over whatever happens to be on PATH)."""
    binary_name = TOOL_BINARY_NAMES[tool_id]
    exe_ext = ".exe" if platform.system() == "Windows" else ""

    if tools_dir is not None:
        candidate = tools_dir / tool_id / f"{binary_name}{exe_ext}"
        if candidate.exists():
            return candidate

    found = shutil.which(f"{binary_name}{exe_ext}") or shutil.which(binary_name)
    return Path(found) if found else None


def resolve_ffprobe(tools_dir: Optional[Path]) -> Optional[Path]:
    """ffprobe is bundled alongside ffmpeg's own binary in MeedyaDL's tools
    directory (tools_dir/ffmpeg/ffprobe[.exe]), not under its own tool_id
    -- see install_companion_ffprobe() in dependency_manager.rs."""
    exe_ext = ".exe" if platform.system() == "Windows" else ""

    if tools_dir is not None:
        candidate = tools_dir / "ffmpeg" / f"ffprobe{exe_ext}"
        if candidate.exists():
            return candidate

    found = shutil.which(f"ffprobe{exe_ext}") or shutil.which("ffprobe")
    return Path(found) if found else None


# ---------------------------------------------------------------------------
# Phase 1: Environment
# ---------------------------------------------------------------------------


def phase_environment(python_bin: Path, expect_version: str) -> tuple[Phase, Optional[str]]:
    """Confirms the resolved interpreter has the expected GAMDL version
    installed, that its CLI entry point runs, and that the compiled
    `gamdl._ammuxer` extension -- where the 3.8.4 song-ending fix lives --
    actually imports on this platform/Python combination (the wheel is
    published as cp310-abi3, but a bad install or an unsupported platform
    -- Linux ARMv7 has no wheel at all -- would silently fall back to an
    older gamdl release without this check)."""
    phase = Phase("Environment")
    installed_version: Optional[str] = None

    try:
        proc = run_cmd([str(python_bin), "-m", "pip", "show", "gamdl"], timeout=30)
    except (OSError, subprocess.TimeoutExpired) as exc:
        phase.add("pip show gamdl runs", False, f"failed to invoke pip: {exc}")
        return phase, None

    if proc.returncode != 0:
        phase.add("pip show gamdl runs", False, f"exit {proc.returncode}: {proc.stderr.strip()}")
    else:
        match = re.search(r"^Version:\s*(\S+)", proc.stdout, re.MULTILINE)
        installed_version = match.group(1) if match else None
        if installed_version is None:
            phase.add("pip show gamdl reports a version", False, "no 'Version:' line in pip show output")
        elif installed_version == expect_version:
            phase.add(f"gamdl version matches expected ({expect_version})", True)
        else:
            phase.add(
                f"gamdl version matches expected ({expect_version})",
                False,
                f"installed: {installed_version}",
            )

    try:
        proc = run_cmd([str(python_bin), "-m", "gamdl", "--version"], timeout=30)
        ok = proc.returncode == 0
        phase.add(
            "python -m gamdl --version runs",
            ok,
            "" if ok else f"exit {proc.returncode}: {proc.stderr.strip()}",
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        phase.add("python -m gamdl --version runs", False, str(exc))

    try:
        proc = run_cmd([str(python_bin), "-c", "import gamdl._ammuxer"], timeout=30)
        ok = proc.returncode == 0
        phase.add(
            "compiled gamdl._ammuxer extension imports (3.8.4 fix lives here)",
            ok,
            "" if ok else proc.stderr.strip(),
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        phase.add("compiled gamdl._ammuxer extension imports (3.8.4 fix lives here)", False, str(exc))

    return phase, installed_version


# ---------------------------------------------------------------------------
# Phase 2: Wrapper preflight
# ---------------------------------------------------------------------------


def phase_wrapper_preflight(
    wrapper_url: str, wrapper_m3u8: str, wrapper_decrypt: str
) -> tuple[Phase, Optional[str]]:
    """Checks that the wrapper-v2 daemon's three sockets are alive and that
    its self-reported version is EXACTLY "0.0.2" -- GAMDL 3.8.2+ hard-
    requires this exact match at CLI startup and aborts on any mismatch
    (see WrapperV2Me's doc comment in health_check_service.rs). The m3u8
    socket is checked here as a daemon-liveness signal even though GAMDL's
    wrapper-v2 CLI (>= 3.6, which includes 3.8.4) only consumes a single
    `--wrapper-url` HTTP endpoint plus the separate decrypt host/port pair
    -- it is NOT passed as its own CLI flag to GAMDL in this script. This
    matches MeedyaDL's own three-address "wrapper triangle" preflight
    convention (check_wrapper_health / check_wrapper_m3u8_health /
    check_wrapper_decrypt_health), which checks all three sockets
    regardless of which subset the currently-detected GAMDL version's CLI
    actually reads."""
    phase = Phase("Wrapper preflight")
    daemon_version: Optional[str] = None

    try:
        host, port = split_host_port(wrapper_m3u8)
        ok, detail = tcp_check(host, port)
    except ValueError as exc:
        ok, detail = False, str(exc)
    phase.add(f"TCP connect to m3u8 socket ({wrapper_m3u8})", ok, detail)

    try:
        host, port = split_host_port(wrapper_decrypt)
        ok, detail = tcp_check(host, port)
    except ValueError as exc:
        ok, detail = False, str(exc)
    phase.add(f"TCP connect to decrypt socket ({wrapper_decrypt})", ok, detail)

    url = wrapper_url.rstrip("/") + "/me"
    try:
        request = urllib.request.Request(url, headers={"User-Agent": "MeedyaDL-smoke-test"})
        with urllib.request.urlopen(request, timeout=5) as response:  # noqa: S310 -- user-supplied local wrapper address, not arbitrary internet input
            body = response.read().decode("utf-8", errors="replace")
        data = json.loads(body)
        daemon_version = data.get("version")
        if daemon_version == "0.0.2":
            phase.add("wrapper daemon GET /me reports version exactly '0.0.2'", True)
        else:
            phase.add(
                "wrapper daemon GET /me reports version exactly '0.0.2'",
                False,
                f"reported: {daemon_version!r}",
            )
    except (urllib.error.URLError, TimeoutError, OSError, json.JSONDecodeError, ValueError) as exc:
        phase.add("wrapper daemon GET /me reports version exactly '0.0.2'", False, str(exc))

    return phase, daemon_version


# ---------------------------------------------------------------------------
# Phase 3: Download matrix
# ---------------------------------------------------------------------------


def build_gamdl_cmd(
    python_bin: Path,
    url: str,
    output_dir: Path,
    codec: str,
    tools: dict[str, Optional[Path]],
    cookies: Optional[Path],
    wrapper: Optional[dict[str, str]],
) -> list[str]:
    """Assembles a `python -m gamdl` invocation for one leg of the matrix.
    Tool paths are only passed when resolved (None is simply omitted,
    letting GAMDL fall back to its own PATH resolution) -- see
    resolve_tool()/resolve_ffprobe()."""
    cmd = [str(python_bin), "-m", "gamdl"]

    for tool_id in ("ffmpeg", "mp4decrypt", "mp4box", "nm3u8dlre"):
        tool_path = tools.get(tool_id)
        if tool_path is not None:
            cmd += [TOOL_CLI_FLAGS[tool_id], str(tool_path)]

    cmd += ["--output-path", str(output_dir)]
    cmd += ["--song-codec-priority", codec]

    if wrapper is not None:
        # Wrapper-v2 (GAMDL >= 3.6, which includes the 3.8.4 target of
        # this harness): a single HTTP base URL plus a separate native
        # TCP decrypt host/port pair (GAMDL >= 3.8.2). Mirrors
        # GamdlOptions::path_cli_args()'s `supports(GamdlFeature::WrapperUrl)`
        # branch in src-tauri/src/models/gamdl_options.rs.
        cmd += ["--use-wrapper", "--wrapper-url", wrapper["url"]]
        decrypt_host, decrypt_port = split_host_port(wrapper["decrypt"])
        cmd += ["--wrapper-decrypt-host", decrypt_host, "--wrapper-decrypt-port", decrypt_port]
    elif cookies is not None:
        cmd += ["--cookies-path", str(cookies)]

    cmd.append(url)
    return cmd


def phase_download_matrix(
    python_bin: Path,
    tools: dict[str, Optional[Path]],
    url: str,
    mode: str,
    cookies: Optional[Path],
    wrapper: Optional[dict[str, str]],
    workdir: Path,
    download_timeout_s: float,
) -> tuple[Phase, dict[str, Path]]:
    """Runs the real GAMDL download(s) for the requested mode(s) into
    dedicated subdirectories of a fresh temp dir, and locates the produced
    audio file for each. `produced` maps a human-readable leg label to the
    file Phase 4 should inspect."""
    phase = Phase("Download matrix")
    produced: dict[str, Path] = {}

    jobs: list[tuple[str, str, Optional[dict[str, str]], Optional[Path]]] = []
    if mode in ("nonwrapper", "both"):
        jobs.append(("nonwrapper-aac", "aac", None, cookies))
    if mode in ("wrapper", "both"):
        # ALAC remains wrapper-dependent on GAMDL 3.8+ (only ALAC does --
        # every other non-web codec moved to the non-wrapper /v1/play/assets
        # path in 3.8). Atmos exercises the OTHER wrapper-dependent decrypt
        # shape (multichannel), giving two independent code paths through
        # the fixed ammuxer extension rather than one.
        jobs.append(("wrapper-alac", "alac", wrapper, None))
        jobs.append(("wrapper-atmos", "atmos", wrapper, None))

    for label, codec, wrapper_args, cookies_args in jobs:
        out_dir = workdir / label
        out_dir.mkdir(parents=True, exist_ok=True)
        cmd = build_gamdl_cmd(
            python_bin, url, out_dir, codec, tools, cookies=cookies_args, wrapper=wrapper_args
        )

        try:
            proc = run_cmd(cmd, timeout=download_timeout_s)
        except subprocess.TimeoutExpired:
            phase.add(f"{label}: gamdl download completes", False, f"timed out after {download_timeout_s:.0f}s")
            continue
        except OSError as exc:
            phase.add(f"{label}: gamdl download completes", False, str(exc))
            continue

        if proc.returncode != 0:
            # Keep the tail of stderr only -- GAMDL's output can be long
            # (progress lines, per-track logging) and the actionable part
            # of a failure is almost always at the end.
            tail = proc.stderr.strip()[-2000:]
            phase.add(f"{label}: gamdl download completes", False, f"exit {proc.returncode}: {tail}")
            continue
        phase.add(f"{label}: gamdl download completes", True)

        files = find_audio_files(out_dir)
        if not files:
            phase.add(f"{label}: gamdl produced an audio file", False, "no audio file found under output directory")
            continue
        phase.add(f"{label}: gamdl produced an audio file", True, str(files[0]))
        produced[label] = files[0]

    return phase, produced


# ---------------------------------------------------------------------------
# Phase 4: Song-ending integrity (THE CORE CHECK)
# ---------------------------------------------------------------------------


def _ffprobe_json(ffprobe_bin: Path, extra_args: list[str], file: Path) -> dict:
    proc = run_cmd(
        [str(ffprobe_bin), "-v", "error", *extra_args, "-print_format", "json", str(file)],
        timeout=60,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or f"ffprobe exited {proc.returncode}")
    return json.loads(proc.stdout)


def _decoded_duration_sec(ffprobe_bin: Path, file: Path) -> float:
    """The ACTUAL decoded sample count divided by sample rate -- the
    ground truth for how long the audio really is, independent of
    whatever the container's metadata *claims* the duration is. A
    truncated/corrupt tail (the 3.8.2/3.8.3 bug) can leave the container's
    declared duration unchanged while the real decodable audio is shorter
    -- which is exactly why this is compared against the container
    duration below, not treated as redundant with it."""
    data = _ffprobe_json(
        ffprobe_bin,
        ["-select_streams", "a:0", "-count_samples", "-show_entries", "stream=nb_read_samples,sample_rate"],
        file,
    )
    stream = data["streams"][0]
    return int(stream["nb_read_samples"]) / int(stream["sample_rate"])


def _container_duration_sec(ffprobe_bin: Path, file: Path) -> float:
    data = _ffprobe_json(ffprobe_bin, ["-show_entries", "format=duration"], file)
    return float(data["format"]["duration"])


def phase_song_integrity(
    files: dict[str, Path],
    ffmpeg_bin: Optional[Path],
    ffprobe_bin: Optional[Path],
    expected_duration_ms: Optional[int],
) -> Phase:
    phase = Phase("Song-ending integrity")

    if ffmpeg_bin is None or ffprobe_bin is None:
        phase.add(
            "ffmpeg and ffprobe available for integrity checks",
            False,
            f"ffmpeg={ffmpeg_bin} ffprobe={ffprobe_bin} -- resolve via --tools-dir or PATH",
        )
        return phase

    if not files:
        phase.add_skip("song-ending integrity", "no downloaded files to check (see Download matrix phase)")
        return phase

    for label, path in sorted(files.items()):
        # (a) Full-file decode must produce zero stderr output. `-f null -`
        # decodes every frame without writing an output file; any decode
        # warning/error (including a corrupt-tail symptom that surfaces as
        # a decode error near EOF) lands on stderr.
        try:
            proc = run_cmd(
                [str(ffmpeg_bin), "-v", "error", "-i", str(path), "-f", "null", "-"], timeout=300
            )
            stderr = proc.stderr.strip()
            ok = proc.returncode == 0 and not stderr
            phase.add(
                f"{label}: full decode produces no ffmpeg errors",
                ok,
                stderr[-1000:] if not ok else "",
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            phase.add(f"{label}: full decode produces no ffmpeg errors", False, str(exc))
            continue  # no point running the rest if ffmpeg itself can't be invoked

        # (b) Decoded sample count vs container-reported duration, within
        # 250ms -- generous enough to absorb AAC encoder priming/padding
        # samples (a well-known, benign few-hundred-sample discrepancy),
        # tight enough to catch a materially truncated file.
        container_sec: Optional[float] = None
        try:
            decoded_sec = _decoded_duration_sec(ffprobe_bin, path)
            container_sec = _container_duration_sec(ffprobe_bin, path)
            delta_ms = abs(decoded_sec - container_sec) * 1000
            ok = delta_ms <= 250
            phase.add(
                f"{label}: decoded sample count matches container duration (within 250ms)",
                ok,
                f"decoded={decoded_sec:.3f}s container={container_sec:.3f}s delta={delta_ms:.1f}ms",
            )
        except (RuntimeError, KeyError, ValueError, ZeroDivisionError) as exc:
            phase.add(
                f"{label}: decoded sample count matches container duration (within 250ms)",
                False,
                str(exc),
            )

        # (c) THE check that specifically targets the 3.8.2/3.8.3 bug:
        # decode only the final 5 seconds (`-sseof -5`). A whole-file
        # decode can sometimes complete "cleanly" even with a corrupt tail
        # if ffmpeg recovers silently; seeking to just before EOF and
        # decoding forward isolates exactly the region the flush-ordering
        # bug corrupted.
        try:
            proc = run_cmd(
                [str(ffmpeg_bin), "-v", "error", "-sseof", "-5", "-i", str(path), "-f", "null", "-"],
                timeout=60,
            )
            stderr = proc.stderr.strip()
            ok = proc.returncode == 0 and not stderr
            phase.add(
                f"{label}: final 5 seconds decode cleanly (catches truncated/corrupt endings -- the 3.8.2/3.8.3 bug)",
                ok,
                stderr[-1000:] if not ok else "",
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            phase.add(
                f"{label}: final 5 seconds decode cleanly (catches truncated/corrupt endings -- the 3.8.2/3.8.3 bug)",
                False,
                str(exc),
            )

        # (d) Optional cross-check against a known-good duration supplied
        # by the operator (e.g. the track's official runtime), within 1s.
        if expected_duration_ms is not None:
            if container_sec is None:
                phase.add(
                    f"{label}: container duration matches --expected-duration-ms (within 1s)",
                    False,
                    "container duration unavailable (ffprobe failed above)",
                )
            else:
                delta_ms = abs(container_sec * 1000 - expected_duration_ms)
                ok = delta_ms <= 1000
                phase.add(
                    f"{label}: container duration matches --expected-duration-ms (within 1s)",
                    ok,
                    f"container={container_sec * 1000:.0f}ms expected={expected_duration_ms}ms delta={delta_ms:.0f}ms",
                )

    return phase


# ---------------------------------------------------------------------------
# Phase 5: Report (TAP + JSON)
# ---------------------------------------------------------------------------


def print_tap(phases: list[Phase]) -> bool:
    """Prints a TAP (Test Anything Protocol)-style report to stdout and
    returns whether every check passed."""
    numbered = [(phase.name, check) for phase in phases for check in phase.checks]

    print("TAP version 13")
    print(f"1..{len(numbered)}")

    all_ok = True
    for n, (phase_name, check) in enumerate(numbered, start=1):
        if not check.ok:
            all_ok = False
        status = "ok" if check.ok else "not ok"
        suffix = " # SKIP" if check.skipped else ""
        print(f"{status} {n} - {phase_name}: {check.name}{suffix}")
        if check.detail:
            for line in check.detail.splitlines() or [check.detail]:
                print(f"  # {line}")

    return all_ok


def build_json_report(phases: list[Phase], meta: dict) -> dict:
    """Builds the machine-readable report. Shaped to be pasted directly
    into a `.github/audits/*.md` file (meta carries exactly the fields
    that section of an audit doc needs: gamdl version, platform triple,
    wrapper daemon version)."""
    return {
        "meta": meta,
        "phases": [
            {
                "name": phase.name,
                "checks": [
                    {"name": c.name, "ok": c.ok, "skipped": c.skipped, "detail": c.detail}
                    for c in phase.checks
                ],
            }
            for phase in phases
        ],
        "summary": {
            "total": sum(len(p.checks) for p in phases),
            "passed": sum(1 for p in phases for c in p.checks if c.ok),
            "failed": sum(1 for p in phases for c in p.checks if not c.ok),
        },
    }


def finish(phases: list[Phase], meta: dict, json_path: Optional[Path]) -> int:
    """Common tail for every code path through main(): print the TAP
    report, optionally write the JSON report, and compute the exit code.
    Always runs -- even a fatal early setup failure (e.g. no Python
    interpreter resolved) goes through here, so a --json report is always
    produced and the script never crashes with an unhandled traceback."""
    all_ok = print_tap(phases)
    report = build_json_report(phases, meta)

    if json_path is not None:
        json_path.parent.mkdir(parents=True, exist_ok=True)
        json_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(f"\nJSON report written to {json_path}")

    return 0 if all_ok else 1


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def parse_args(argv: Optional[list[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="gamdl_live_smoke.py",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        description=(
            "Live GAMDL smoke test -- verifies a real download decodes cleanly "
            "end-to-end, focused on the 3.8.2/3.8.3 wrapper-decrypt song-ending "
            "truncation bug fixed in 3.8.4. Requires real credentials and real "
            "content; NEVER run this in CI. See scripts/smoke-tests/README.md."
        ),
    )

    python_group = parser.add_mutually_exclusive_group()
    python_group.add_argument(
        "--python-bin",
        type=Path,
        default=None,
        help="Path to a Python 3.10+ interpreter with gamdl installed.",
    )
    python_group.add_argument(
        "--auto",
        action="store_true",
        help=(
            "Probe MeedyaDL's managed app-data directories for the managed "
            "interpreter (handles both the portable and system-Python-venv "
            "layouts, including Windows' venv Scripts\\python.exe). This is "
            "the default when neither --python-bin nor --auto is given."
        ),
    )

    parser.add_argument(
        "--tools-dir",
        type=Path,
        default=None,
        help=(
            "MeedyaDL-style tools directory (tools/{ffmpeg,mp4decrypt,nm3u8dlre,mp4box}/...). "
            "Any tool not found there falls back to the system PATH."
        ),
    )
    parser.add_argument("--url", required=True, help="A single Apple Music SONG URL.")
    parser.add_argument(
        "--mode",
        choices=("nonwrapper", "wrapper", "both"),
        default="both",
        help="Which auth path(s) to exercise (default: both).",
    )
    parser.add_argument(
        "--cookies", type=Path, default=None, help="Netscape cookies.txt file (required for nonwrapper/both)."
    )
    parser.add_argument(
        "--wrapper-url",
        default=None,
        help="Wrapper-v2 HTTP base URL, e.g. http://127.0.0.1:30020 (required for wrapper/both).",
    )
    parser.add_argument(
        "--wrapper-m3u8",
        default=None,
        help=(
            "Wrapper m3u8 host:port, e.g. 127.0.0.1:20020 (required for wrapper/both). "
            "Health-checked only -- not forwarded to GAMDL's CLI on wrapper-v2 (>= 3.6), "
            "which reads the m3u8 URL via --wrapper-url instead."
        ),
    )
    parser.add_argument(
        "--wrapper-decrypt",
        default=None,
        help="Wrapper decrypt host:port, e.g. 127.0.0.1:10020 (required for wrapper/both).",
    )
    parser.add_argument(
        "--expected-duration-ms",
        type=int,
        default=None,
        help="Optional known-good track duration in milliseconds, cross-checked within 1s.",
    )
    parser.add_argument(
        "--install",
        action="store_true",
        help=f"pip install --upgrade 'gamdl>=3.0,<={DEFAULT_GAMDL_SPEC_CEILING}' before running the checks.",
    )
    parser.add_argument(
        "--expect-version",
        default=DEFAULT_GAMDL_SPEC_CEILING,
        help=f"Expected installed GAMDL version (default: {DEFAULT_GAMDL_SPEC_CEILING}).",
    )
    parser.add_argument(
        "--json",
        dest="json_path",
        type=Path,
        default=None,
        help="Write the machine-readable report to this path.",
    )
    parser.add_argument(
        "--download-timeout",
        type=float,
        default=900.0,
        help="Per-download timeout in seconds (default: 900 = 15 minutes).",
    )
    parser.add_argument(
        "--keep-temp",
        action="store_true",
        help="Do not delete the temporary download directory on exit (needed for the ears-on tail listen).",
    )

    return parser.parse_args(argv)


def main(argv: Optional[list[str]] = None) -> int:
    args = parse_args(argv)

    # --auto is the implicit default when the caller specified neither
    # --python-bin nor --auto (the mutually-exclusive group above only
    # prevents specifying BOTH, it doesn't require one).
    if args.python_bin is None and not args.auto:
        args.auto = True

    # Validate mode-specific required arguments up front, before touching
    # the network or the filesystem, so a misconfigured invocation fails
    # fast with a clear message instead of partway through a phase.
    missing: list[str] = []
    if args.mode in ("nonwrapper", "both") and args.cookies is None:
        missing.append("--cookies is required for --mode nonwrapper/both")
    if args.mode in ("wrapper", "both"):
        if not args.wrapper_url:
            missing.append("--wrapper-url is required for --mode wrapper/both")
        if not args.wrapper_m3u8:
            missing.append("--wrapper-m3u8 is required for --mode wrapper/both")
        if not args.wrapper_decrypt:
            missing.append("--wrapper-decrypt is required for --mode wrapper/both")
    if missing:
        for message in missing:
            print(f"error: {message}", file=sys.stderr)
        return 2

    meta: dict = {
        "mode": args.mode,
        "expect_version": args.expect_version,
        "platform_triple": platform_triple(),
        "harness_python_version": sys.version.split()[0],
        "generated_at": _dt.datetime.now(_dt.timezone.utc).isoformat(),
        "url": args.url,
    }

    phases: list[Phase] = []
    setup_phase = Phase("Setup")
    phases.append(setup_phase)

    # --- Resolve the target Python interpreter ---
    python_bin = args.python_bin
    if args.auto and python_bin is None:
        python_bin = resolve_auto_python()

    if python_bin is None or not Path(python_bin).exists():
        setup_phase.add(
            "python interpreter resolved",
            False,
            f"--python-bin={args.python_bin} --auto={args.auto} resolved to: {python_bin}",
        )
        # Nothing downstream can run without an interpreter -- report and
        # exit. This is also the path exercised by a network-free --json
        # dry-run (see scripts/smoke-tests/README.md).
        return finish(phases, meta, args.json_path)

    setup_phase.add("python interpreter resolved", True, str(python_bin))
    meta["python_bin"] = str(python_bin)

    # --- Resolve external tools ---
    tools: dict[str, Optional[Path]] = {
        "ffmpeg": resolve_tool(args.tools_dir, "ffmpeg"),
        "mp4decrypt": resolve_tool(args.tools_dir, "mp4decrypt"),
        "mp4box": resolve_tool(args.tools_dir, "mp4box"),
        "nm3u8dlre": resolve_tool(args.tools_dir, "nm3u8dlre"),
    }
    ffprobe_bin = resolve_ffprobe(args.tools_dir)
    for tool_id, tool_path in tools.items():
        setup_phase.add(
            f"{tool_id} resolved",
            tool_path is not None,
            str(tool_path) if tool_path else "not found in --tools-dir or PATH",
        )
    setup_phase.add(
        "ffprobe resolved",
        ffprobe_bin is not None,
        str(ffprobe_bin) if ffprobe_bin else "not found in --tools-dir or PATH",
    )

    # --- Optional: upgrade gamdl first ---
    if args.install:
        install_phase = Phase("Install")
        spec = f"gamdl>=3.0,<={DEFAULT_GAMDL_SPEC_CEILING}"
        try:
            proc = run_cmd([str(python_bin), "-m", "pip", "install", "--upgrade", spec], timeout=600)
            ok = proc.returncode == 0
            install_phase.add(
                f"pip install --upgrade '{spec}'", ok, "" if ok else proc.stderr.strip()[-2000:]
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            install_phase.add(f"pip install --upgrade '{spec}'", False, str(exc))
        phases.append(install_phase)

    # --- Phase 1: Environment ---
    env_phase, installed_version = phase_environment(python_bin, args.expect_version)
    phases.append(env_phase)
    meta["gamdl_version"] = installed_version

    # --- Phase 2: Wrapper preflight (wrapper/both modes only) ---
    wrapper_args: Optional[dict[str, str]] = None
    if args.mode in ("wrapper", "both"):
        wrapper_phase, daemon_version = phase_wrapper_preflight(
            args.wrapper_url, args.wrapper_m3u8, args.wrapper_decrypt
        )
        phases.append(wrapper_phase)
        meta["wrapper_daemon_version"] = daemon_version
        wrapper_args = {"url": args.wrapper_url, "decrypt": args.wrapper_decrypt}

    # --- Phases 3 & 4: real downloads + song-ending integrity ---
    workdir = Path(tempfile.mkdtemp(prefix="meedyadl-gamdl-smoke-"))
    meta["workdir"] = str(workdir)
    try:
        download_phase, produced_files = phase_download_matrix(
            python_bin,
            tools,
            args.url,
            args.mode,
            args.cookies,
            wrapper_args,
            workdir,
            args.download_timeout,
        )
        phases.append(download_phase)

        integrity_phase = phase_song_integrity(
            produced_files, tools["ffmpeg"], ffprobe_bin, args.expected_duration_ms
        )
        phases.append(integrity_phase)
    finally:
        if args.keep_temp:
            print(f"\n--keep-temp set -- downloaded files left at: {workdir}")
        else:
            shutil.rmtree(workdir, ignore_errors=True)

    return finish(phases, meta, args.json_path)


if __name__ == "__main__":
    sys.exit(main())
