# Wrapper authentication

> **Quick orientation:** MeedyaDL supports **two completely different
> wrappers** depending on which GAMDL release is installed. Both are
> opt-in — cookie-only mode works without either.

| You're on … | Wrapper to install | Default endpoint |
|---|---|---|
| GAMDL **2.9.1 – 3.5.x** | **wrapper-v1** ([WorldObservationLog/wrapper](https://github.com/WorldObservationLog/wrapper)) | Three sockets: HTTP `127.0.0.1:30020` + TCP `127.0.0.1:20020` + TCP `127.0.0.1:10020` |
| GAMDL **3.6 and newer** | **wrapper-v2** ([glomatico/wrapper-v2](https://github.com/glomatico/wrapper-v2)) | One HTTP endpoint: `http://127.0.0.1` (port 80) |

You can check which GAMDL version MeedyaDL has installed in
**Settings → System → Component Versions**.

---

## When you actually need a wrapper

Most users don't. The catalog API, song metadata, lyrics, artwork, and
the entire `aac-web` / `aac-he-web` codec family all work with cookie-only
authentication. Wrapper auth is required for:

- **ALAC** (lossless), **Atmos** (Dolby Atmos), **AC3** (Dolby Digital).
- **Music videos** (on certain regional content).
- The pre-3.6 `aac` / `aac-he` / `aac-binaural` / `aac-downmix` codec
  variants (the ones that don't end in `-web`).
- The full set of audio traits / spatial audio flags on certain albums.

If your music library is fine with lossy AAC at 256 kbps, you can skip
wrapper setup entirely — MeedyaDL will use `aac-web` on GAMDL 3.6+ and
fall back gracefully on older releases.

---

## wrapper-v1 (GAMDL ≤ 3.5.x)

The original wrapper is a native binary (Windows / macOS / Linux ports
exist) that exposes three local sockets. MeedyaDL talks to it via the
three Settings fields in **Settings → Advanced → Wrapper**:

| Setting | Default | Purpose |
|---|---|---|
| Wrapper account URL | `http://127.0.0.1:30020` | Apple ID login, MusicKit JWT issuance |
| Wrapper m3u8 IP | `127.0.0.1:20020` | HLS master playlist URL fetch (GAMDL 3.1+) |
| Wrapper decryption IP | `127.0.0.1:10020` | FairPlay sample decrypt socket (#743) |

Setup is documented at [WorldObservationLog/wrapper](https://github.com/WorldObservationLog/wrapper).
MeedyaDL probes all three sockets before a wrapper download starts and
surfaces a yellow toast if any is unreachable.

---

## wrapper-v2 (GAMDL 3.6+)

[wrapper-v2](https://github.com/glomatico/wrapper-v2) is a complete
rewrite: a single C++ daemon (built with the Android NDK) that exposes
**one HTTP REST API** instead of three sockets. Its endpoints are:

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/health` | Liveness probe — checked by MeedyaDL preflight |
| `GET` | `/me` | Auth state + runtime readiness — checked by MeedyaDL preflight |
| `POST` | `/login` | Apple ID sign-in |
| `POST` | `/login/2fa` | HSA2 second factor |
| `GET` | `/playback` | Apple Music playback dispatch (replaces the m3u8 socket) |
| `POST` | `/decrypt` | FairPlay sample decrypt batch (binary protocol) |
| `DELETE` | `/login` | Clear cached tokens |

In MeedyaDL, **Settings → Advanced → Wrapper** shows a single **Wrapper
URL** field (default `http://127.0.0.1`) when the installed GAMDL is
3.6 or newer. The three v1 fields stay in the settings file for users
who downgrade GAMDL but are hidden in the UI.

### Important deployment difference vs wrapper-v1

The wrapper-v2 daemon runs **inside a Linux chroot** and depends on
Apple Music for Android's native `.so` libraries. This has practical
consequences:

- **Linux**: Native run is possible but the process needs `SYS_ADMIN`
  / `SYS_CHROOT` / `SYS_PTRACE` privileges (root). You must extract
  Apple's `.so` files from the Apple Music Android APK and stage them
  into the wrapper's `rootfs/system/lib64/` (digests pinned in
  `LIBS_VERSION.json`).
- **macOS / Windows**: **Docker Desktop is required.** The wrapper-v2
  ships as a `Dockerfile` + `compose.yaml`; you build the image
  locally with the staged `.so` files (Apple `.so` files are not
  redistributed by glomatico/wrapper-v2 — you stage them yourself).

> **MeedyaDL does NOT bundle wrapper-v2.** Three blockers:
> 1. Distributing Apple's proprietary `.so` files inside our installer
>    would be a licensing nightmare.
> 2. The Linux chroot mode needs root capabilities that MeedyaDL
>    doesn't have on user installs.
> 3. Cross-compilation via NDK isn't viable as a one-click MeedyaDL
>    bundle.

### First-time setup outline

1. Clone https://github.com/glomatico/wrapper-v2.
2. Extract Apple Music for Android's `.so` files (the upstream README
   has instructions and `tools/extract-libs.sh` helps verify SHA-256s).
3. `docker compose build` (Linux/macOS/Windows).
4. `docker compose up -d` to start the daemon — exposes
   `http://localhost:80` by default.
5. In MeedyaDL, **Settings → Advanced → Wrapper**:
   - Toggle **Use wrapper** on.
   - Set **Wrapper URL** to `http://127.0.0.1` (or whatever port you
     mapped in `compose.yaml`).
   - The "Sign In" button calls `POST /login` with the Apple ID +
     password you provide — store nothing in MeedyaDL.

### Why the "Sign In" button matters

GAMDL 3.6 added an **interactive credential prompt** at the CLI level
(`gamdl/cli/interactive_prompts.py`). When MeedyaDL spawns GAMDL as a
non-interactive subprocess, an unauthenticated wrapper would cause
GAMDL to **deadlock waiting on stdin** — the download would never
progress and the queue would stall until the watchdog timed out at
20 min.

MeedyaDL preflights `GET /me` before every wrapper download and
surfaces a yellow toast if the wrapper is logged-out, telling you to
sign in via Settings before the download will proceed. This avoids
the deadlock entirely.

---

## Cookie-only mode (no wrapper)

If you don't want to run either wrapper, MeedyaDL still works with
**cookies only**. You'll need:

- A valid Apple Music subscription
- An exported cookies file (see
  [Cookie management](./cookie-management.md))
- Acceptance of these limitations:
  - **Lossy AAC only** (256 kbps `aac-web` on GAMDL 3.6+, or
    `aac-legacy` on older releases)
  - No ALAC / Atmos / AC3 / Spatial audio
  - Music videos: hit-and-miss

This is the recommended mode for casual users.

---

## Troubleshooting

**"Wrapper-v2 daemon at http://127.0.0.1 returned HTTP …" on every
download** → Container isn't running or isn't bound to the URL.
Check `docker compose ps` and the `HTTP_PORT` env var in your
`compose.yaml`.

**"Wrapper-v2 daemon at … is reachable but not signed in" toast every
download** → Hit **Sign In** in Settings → Advanced → Wrapper. If
you've set `WRAPPER_USERNAME` / `WRAPPER_PASSWORD` env vars on the
container, the daemon should restore the session automatically on
restart — check `docker logs wrapper-v2` for restore errors.

**Activity log shows `Wrapper-v2 daemon at … unreachable` toast on
macOS even though `docker ps` shows it running** → On macOS, Docker
Desktop port mappings sometimes don't bind to `127.0.0.1` cleanly
when the daemon restarts. Try `http://localhost` instead, or set
`docker compose down && docker compose up -d`.

**Downloads stall forever with "Companion: downloading aac (tier N)…
— X min elapsed"** → GAMDL 3.6 needs wrapper-v2 for the non-aac-web
codec families. If you don't have wrapper-v2 set up, switch the
companion mode to one that uses `aac-web` (e.g. **SpecialistToLossy**
on the Apple Music side) or stay on GAMDL 3.5.2 until you have
wrapper-v2 working.
