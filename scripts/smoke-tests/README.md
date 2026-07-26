# GAMDL live smoke test

`gamdl_live_smoke.py` is a stdlib-only Python 3.10+ script that runs a
**real** GAMDL download end-to-end and inspects the resulting audio file
for the specific data-corruption bug that shipped in GAMDL 3.8.2/3.8.3 and
was fixed in 3.8.4: queued decrypted samples weren't flushed before
immediate writes on the wrapper-decrypt path, so payload order didn't
always match the container's sample-size table — the audible symptom is a
**truncated or corrupt song ending**. MeedyaDL's tooling now recommends
GAMDL 3.8.4, but until this script exists nobody had actually run the
live verification gate that closes the loop between "the upstream fix
landed" and "we confirmed it works on our supported platforms."

**This script is intentionally NOT wired into any CI workflow, and never
should be.** It needs a real Apple Music subscription (cookies and/or a
running wrapper daemon), a real track to download, and — for the ALAC
wrapper leg specifically — a human's ears on the final few seconds. None
of that is available to CI. Run it by hand, on demand, whenever a new
GAMDL release needs a pre-stable live-verification pass (see CLAUDE.md's
GAMDL version-audit entries for the cadence this has followed historically).

## Prerequisites

- **Python**: 3.10+ (the same floor MeedyaDL's own managed-Python
  detection uses — GAMDL's `_ammuxer` wheel is `cp310-abi3`). Either the
  app's own managed interpreter (`--auto` or an explicit `--python-bin`)
  or any compatible interpreter with `gamdl` installed.
- **External tools**: FFmpeg (with `ffprobe`), `mp4decrypt`, MP4Box, and
  N_m3u8DL-RE. Pass `--tools-dir` pointing at a MeedyaDL-managed
  `{app_data}/tools/` directory to reuse the app's own binaries, or leave
  it unset to fall back to whatever's on `PATH`.
- **Real credentials**, one or both of:
  - a Netscape-format `cookies.txt` exported from a signed-in Apple Music
    session (non-wrapper leg), and/or
  - a running wrapper-v2 daemon with a real, logged-in Apple Music
    subscription, reachable at its account URL, m3u8 socket, and decrypt
    socket (wrapper leg — required for ALAC and Dolby Atmos).
- **A track to test with.** Pick something you don't mind re-downloading
  a few times; a mid-length song (3-5 minutes) is a good default — long
  enough that a truncated-ending bug has room to manifest, short enough
  that the matrix doesn't take forever.

## What it checks

1. **Environment** — the resolved interpreter has exactly the expected
   GAMDL version installed (`pip show gamdl`), the CLI runs
   (`python -m gamdl --version`), and the compiled `gamdl._ammuxer`
   extension — where the 3.8.4 fix actually lives — imports successfully.
2. **Wrapper preflight** (wrapper/both modes only) — TCP-connects to the
   m3u8 and decrypt sockets (3s timeout each), then does an HTTP
   `GET /me` against the wrapper's account URL and asserts the daemon
   reports **exactly** version `0.0.2` (GAMDL 3.8.2+ hard-requires this
   and aborts at startup on any mismatch).
3. **Download matrix** — real downloads into a fresh temp directory:
   - non-wrapper leg: `aac` (works without wrapper auth on GAMDL 3.8+),
   - wrapper leg: `alac` (still wrapper-dependent on 3.8.4) **and**
     `atmos` (a second, independent wrapper-decrypt code path).
4. **Song-ending integrity** — the core check, run per downloaded file:
   - a full `ffmpeg -f null -` decode produces zero stderr output,
   - the decoded sample count (`nb_read_samples / sample_rate`) matches
     the container's reported duration within 250ms (AAC encoder
     priming/padding tolerance),
   - a **tail decode** (`ffmpeg -sseof -5 -f null -`, just the final 5
     seconds) is clean — this is what specifically catches the
     3.8.2/3.8.3 truncated/corrupt-ending bug, since a whole-file decode
     can sometimes "succeed" even with a damaged tail,
   - if `--expected-duration-ms` was supplied, the container duration is
     within 1s of it.
5. **Report** — a TAP-style pass/fail printed to stdout, plus an optional
   machine-readable JSON report (`--json report.json`) shaped so its
   `meta` block (GAMDL version, platform triple, wrapper daemon version)
   and per-check results can be pasted directly into a
   `.github/audits/*.md` write-up.

## Usage

```bash
# Non-wrapper only (cookies-based auth), AAC:
python3 gamdl_live_smoke.py --auto --tools-dir /path/to/tools \
    --url "https://music.apple.com/us/song/example/1234567890" \
    --mode nonwrapper --cookies ./cookies.txt --json report.json

# Wrapper only (ALAC + Dolby Atmos), against a locally running wrapper-v2:
python3 gamdl_live_smoke.py --auto --tools-dir /path/to/tools \
    --url "https://music.apple.com/us/song/example/1234567890" \
    --mode wrapper \
    --wrapper-url http://127.0.0.1:30020 \
    --wrapper-m3u8 127.0.0.1:20020 \
    --wrapper-decrypt 127.0.0.1:10020 \
    --json report.json

# Both legs, with a known-good duration cross-check, keeping the files
# around afterward for the ears-on tail listen:
python3 gamdl_live_smoke.py --auto --tools-dir /path/to/tools \
    --url "https://music.apple.com/us/song/example/1234567890" \
    --mode both --cookies ./cookies.txt \
    --wrapper-url http://127.0.0.1:30020 \
    --wrapper-m3u8 127.0.0.1:20020 --wrapper-decrypt 127.0.0.1:10020 \
    --expected-duration-ms 214000 --json report.json --keep-temp
```

`--auto` probes MeedyaDL's own managed app-data directories
(`{app_data}/python/...`) and understands both on-disk layouts a managed
install can have — the portable `python-build-standalone` layout
(`bin/python3` on macOS/Linux, root `python.exe` on Windows) and the
system-Python-venv layout from issue #1017 (`Scripts\python.exe` on
Windows). Pass `--python-bin` instead to point at any other interpreter
with `gamdl` installed.

Run `--install` to `pip install --upgrade 'gamdl>=3.0,<=3.8.4'` first if
you need to get onto 3.8.4 before running the checks.

## Platform matrix

| Platform | Priority | Notes |
| --- | --- | --- |
| macOS ARM64 | **Required** | Primary supported desktop platform. |
| Windows x64 | **Required** | |
| Linux x64 | **Required** | |
| Windows ARM64 | Nice-to-have | Native ARM64 build; run if you have hardware. |
| Linux aarch64 | Nice-to-have | Pi 4/5, ARM servers. |
| Linux ARMv7 | **Excluded** | No `gamdl._ammuxer` wheel is published for this target — `pip`'s `--only-binary=gamdl` resolves down to **3.8.1** there, which does not contain the fix this script verifies. Running this harness on ARMv7 would test the wrong version; skip it. |

## What genuinely needs a human

This script cannot fully automate the verification — a few things need a
person, on purpose:

- **Cookies.** A valid, signed-in Apple Music `cookies.txt` export is
  personal credential material; there's no way to fabricate one, and it
  shouldn't be checked in or shared.
- **A running wrapper daemon with a real subscription.** The wrapper leg
  (ALAC, Dolby Atmos) needs an actual logged-in wrapper-v2 instance
  reachable over the network — this script only verifies the daemon is
  *reachable and the right version*, it does not stand one up.
- **Choosing the track.** Pick something representative — normal length,
  no unusual container quirks — and ideally something you already know
  the correct duration of (for `--expected-duration-ms`).
- **One ears-on listen.** After a wrapper-decrypted ALAC download passes
  every automated check, actually listen to the final 10-15 seconds. The
  automated checks (clean decode, sample-count match, clean tail decode)
  are strong signals, but "the audio codec didn't error" and "the audio
  sounds correct to a human" are not logically identical — the whole
  point of this harness is restoring confidence after a data-corruption
  bug, and that confidence is only complete once someone has actually
  heard the fix working. Use `--keep-temp` to leave the downloaded files
  in place for this step (the path is printed at the end of the run).

## Interpreting a failure

Every check is independent and reported individually — a single failed
check does not stop the rest of the matrix from running. Read the JSON
report's `summary` block for the pass/fail counts, and the per-check
`detail` field for the specific error. A failure in **Song-ending
integrity** (Phase 4), specifically the "final 5 seconds decode cleanly"
check, is the one this whole script exists to catch — treat it as a
release blocker for the GAMDL version under test, not a flaky test to
retry.
