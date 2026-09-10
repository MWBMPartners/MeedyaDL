<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Wrapper authentication

> **Quick orientation:** MeedyaDL supports **two completely different
> wrappers** depending on which GAMDL release is installed. Both are
> opt-in — cookie-only mode works without either.

| You're on … | Wrapper to install | Default endpoint |
|---|---|---|
| GAMDL **3.0 – 3.5.x** | **wrapper-v1** ([WorldObservationLog/wrapper](https://github.com/WorldObservationLog/wrapper)) | Three sockets: HTTP `127.0.0.1:30020` + TCP `127.0.0.1:20020` + TCP `127.0.0.1:10020` |
| GAMDL **3.6 and newer** | **wrapper-v2** ([glomatico/wrapper-v2](https://github.com/glomatico/wrapper-v2)) | One HTTP endpoint: `http://127.0.0.1` (port 80) |

You can check which GAMDL version MeedyaDL has installed in
**Settings → System → Component Versions**.

---

## Upgrading from GAMDL v2 (support dropped)

MeedyaDL now supports **GAMDL v3.x only** — GAMDL v2 support was dropped.
If you're still on a GAMDL v2 release, MeedyaDL flags it as unsupported and
downloads may fail. **Upgrade GAMDL** — which version to pick depends on
your wrapper setup:

- **If you use wrapper-v1** (the three-socket wrapper): upgrade to
  **GAMDL v3.5** — the *last* release that works with wrapper-v1. Going
  past 3.5 (to 3.6+) requires migrating to **wrapper-v2**, a Docker-based
  setup with extra manual steps (see below), so stay on 3.5 until you're
  ready for that.
- **If you don't use a wrapper** (cookie-only): upgrade straight to the
  **recommended latest** (3.8.5). On GAMDL 3.8+, every non-web codec
  *except ALAC* downloads without any wrapper at all (see the next
  section) — so cookie-only users get the best experience on the newest
  release.

> **Heads-up on the future:** GAMDL v3.5 (and wrapper-v1) won't be
> supported forever. A later MeedyaDL release may raise the floor past
> 3.5, at which point continuing to use wrapper-gated codecs (ALAC, and
> on older GAMDL also Atmos/AC3) will require **wrapper-v2** — a Docker
> container plus extracting Apple Music for Android's `.so` libraries. If
> you rely on wrapper-v1 today, it's worth planning that migration.

---

## When you actually need a wrapper

Most users don't. The catalog API, song metadata, lyrics, artwork, and
the entire `aac-web` / `aac-he-web` codec family all work with cookie-only
authentication. **What still needs a wrapper depends on your GAMDL version:**

**GAMDL 3.8 and newer — wrapper needed for ALAC only.** GAMDL 3.8
introduced a new HLS asset endpoint that unlocks every non-web codec —
`aac`, `aac-he`, `aac-binaural`, `aac-downmix`, and even **Atmos** and
**AC3** — *without* a wrapper. On 3.8+ the only codec that still requires
wrapper auth is **ALAC** (lossless). (3.8.1 further fixed some songs that
previously failed on non-web codecs.) If you don't need ALAC lossless, you
can skip wrapper setup entirely on 3.8+.

**GAMDL 3.0 – 3.7.x — wrapper needed for the full non-web set.** On these
releases wrapper auth is required for:

- **ALAC** (lossless), **Atmos** (Dolby Atmos), **AC3** (Dolby Digital).
- **Music videos** (on certain regional content).
- The `aac` / `aac-he` / `aac-binaural` / `aac-downmix` codec variants
  (the ones that don't end in `-web`).
- The full set of audio traits / spatial audio flags on certain albums.

If your music library is fine with lossy AAC at 256 kbps, you can skip
wrapper setup entirely on any supported version — MeedyaDL uses `aac-web`
and falls back gracefully.

---

## wrapper-v1 (GAMDL ≤ 3.5.x)

The original wrapper is a native program that exposes three local sockets.
Which platforms it has builds for is a question for that project — see
"Platform support" below. MeedyaDL talks to it via the
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
| `POST` | `/decrypt` | FairPlay sample decrypt batch (binary protocol) — **wrapper-v2 0.0.1 only** (GAMDL 3.6 – 3.8.1). On wrapper-v2 **0.0.2** (GAMDL 3.8.2+), decrypt moves to a raw **TCP** port (default `10020`), not this HTTP route — see "GAMDL ↔ wrapper-v2 version lockstep" below. |
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

### Running wrapper-v2 on a separate machine (LAN deployment)

A common request — and a fully supported configuration — is to run
wrapper-v2 on a **different machine** than MeedyaDL. Two reasons:

- You have an always-on home server (Raspberry Pi 4/5, NAS,
  ProxmoxVE box, mini PC) and don't want to keep Docker Desktop
  running on your laptop just for FairPlay decryption.
- One household wrapper instance can serve multiple MeedyaDL
  clients without each one needing its own Docker setup.

This works **out of the box** because wrapper-v2 binds to `0.0.0.0`
by default (the source ships `kDefaultHost = "0.0.0.0"` in
`src/daemon/main.cpp`). Set up:

1. Install wrapper-v2 on the remote host following the standard
   Docker walkthrough above. Confirm it answers
   `curl http://<remote-ip>:80/health` from another machine on
   the same LAN.
2. On the MeedyaDL host, **Settings → Advanced → Wrapper → Wrapper
   URL**: paste the LAN URL (e.g. `http://192.168.1.50` or
   `http://nas.home:80`).
3. Hit **Sign In** as usual — the credentials flow through the LAN
   to the remote wrapper's `/login` endpoint.
4. Queue a download. The pre-flight health check probes the LAN URL
   the same way it probes loopback (3-second TCP/HTTP timeout).

### Security caveat — wrapper-v2 has no network-layer auth

There's an important asymmetry between loopback and LAN deployment:

> **Wrapper-v2 exposes no API key, no bearer token, no CORS check,
> and no client IP allowlist.** Any device that can reach its HTTP
> port can call `/playback` and `/decrypt` against your Apple Music
> credentials.

This is **fine** on:

- **Loopback** (`127.0.0.1`) — only this machine can reach it
- **A trusted home network** behind a residential firewall — you
  control which devices are on the network

This is **risky** on:

- **Shared networks** (coffeeshop wifi, dorm, work LAN, AirBnB) — any
  other client on the network can hit your wrapper
- **Public IPs** — anyone on the internet can hit your wrapper

MeedyaDL surfaces a corresponding hint in **Settings → Advanced →
Wrapper** when the Wrapper URL is non-loopback:

- **Amber note** for private-range IPs / DNS names — informational,
  call out the no-auth caveat
- **Red note** for public IPs — almost always a misconfiguration

### Mitigations if you need stronger isolation

If you want the convenience of running wrapper-v2 elsewhere AND
network-layer protection, you have several options:

1. **Firewall the wrapper port to specific source IPs.** On Linux,
   `iptables -A INPUT -p tcp --dport 80 -s 192.168.1.10 -j ACCEPT`
   then `iptables -A INPUT -p tcp --dport 80 -j DROP`. Equivalent
   on macOS via `pf`, on Windows via Windows Firewall.

2. **Bind wrapper-v2 to loopback on the remote host + SSH tunnel
   from MeedyaDL.** On the wrapper host:
   `WRAPPER_HOST=127.0.0.1 docker compose up -d`. On the MeedyaDL
   host: `ssh -N -L 30020:127.0.0.1:80 user@wrapper-host`. Set
   MeedyaDL's Wrapper URL to `http://127.0.0.1:30020`. The wrapper
   stays loopback-only; the SSH tunnel terminates on the wrapper
   host's loopback.

3. **VLAN segmentation.** If you have a managed switch / router that
   supports VLANs, put the wrapper on a dedicated VLAN with only
   your MeedyaDL machines as authorised members.

4. **VPN-only access.** Run the wrapper on a host accessible only
   via your Tailscale / WireGuard / OpenVPN network — public IP +
   firewall blocking everything except the VPN endpoint.

For a single trusted home network (the most common case), no
mitigation is needed — the LAN itself is the trust boundary.

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

**LAN-deployed wrapper unreachable from MeedyaDL but reachable via
`curl` from the same machine** → Almost always a firewall on the
wrapper host blocking inbound port 80 from non-loopback sources.
Verify with `curl http://<wrapper-lan-ip>:80/health` **from a third
machine** on the LAN. If that works, the issue is in MeedyaDL's
network config — try `ping <wrapper-lan-ip>` from the MeedyaDL host
to rule out routing. If the third-machine `curl` also fails, the
wrapper is bound to loopback only (`WRAPPER_HOST=127.0.0.1`) or the
host's firewall is rejecting the LAN traffic.

**Amber "Wrapper is on your LAN" note in Settings is unexpected** →
That's the #891 security hint. If your wrapper URL is intentionally
on a private-range IP (`10/8`, `172.16-31/12`, `192.168/16`), the
note is just confirming you understand wrapper-v2 has no
network-layer auth. See the "Security caveat" section above for
mitigations if you need stronger isolation.

---

## Platform support

MeedyaDL shows the wrapper settings on every platform it runs on. It does
not check what you are running, and it does not need to: all it ever does
is talk to three addresses you give it, and those addresses can point at
this machine or at another one.

What varies is whether a wrapper you can run **on this machine** exists for
your platform at all. That is decided by the wrapper projects, not by
MeedyaDL:

- **wrapper-v1** is a native program. Which platforms it has builds for is
  a question for that project — check its own releases page before assuming
  one exists for yours.
- **wrapper-v2** is documented above: Linux can run it directly, while
  macOS and Windows need Docker Desktop.

If there is no wrapper you can run here, you can still use one. See below.

### Using a wrapper running somewhere else

If you cannot run a wrapper on this machine, you can point MeedyaDL at one
running elsewhere:

1. **On another machine** — run the wrapper on any machine that can host
   it (a spare box, a home server, a small VPS, a Raspberry Pi) and point
   MeedyaDL at that machine's address.
2. **In Docker** — run it in a container, which works on any host that has
   Docker. wrapper-v2 ships a Docker setup; see the section above.
3. **By editing the settings file** — open MeedyaDL's `settings.json` in the
   app data folder and set `"use_wrapper": true` along with the three
   addresses below.

**Point all three addresses at the same machine, not just one.** This is
the single most common reason a remote wrapper "does not work". MeedyaDL
makes three separate connections, and each has its own setting:

| Setting | Default | What it is for |
|---|---|---|
| `wrapper_account_url` | `http://127.0.0.1:30020` | Signing in and getting a token |
| `wrapper_m3u8_ip` | `127.0.0.1:20020` | Fetching the playlist address (GAMDL 3.1 and later) |
| `wrapper_decrypt_ip` | `127.0.0.1:10020` | The decryption connection |

If you change only the first one, signing in appears to work and the
download then fails partway through — because the other two are still
trying to reach this machine, where nothing is listening. All three are
also in **Settings → Advanced → Wrapper**, which is easier than editing the
file by hand.

---

## Setting it up

### How it works

1. A **wrapper service** runs on your computer (typically at `http://127.0.0.1:30020`)
2. MeedyaDL connects to the wrapper instead of using cookies
3. The wrapper handles Apple ID login, DRM key exchange, and decryption on your behalf

### 1. Obtain and run the wrapper service

The wrapper is a separate application that you run locally. It listens on `http://127.0.0.1:30020` by default. You will need to source this separately — it is not bundled with MeedyaDL.

### 2. Enable the wrapper in MeedyaDL

Go to **Settings > Advanced** and enable the **Use Wrapper** toggle. The default URL (`http://127.0.0.1:30020`) should work if the wrapper is running locally with default settings.

### 3. Configure the URL (if needed)

If your wrapper runs on a different port or host, update the **Wrapper Account URL** field in Settings > Advanced.

---

## Checking it is working

MeedyaDL checks whether the wrapper is reachable in two ways:

### Manual test

In **Settings > Advanced**, click the **Test Connection** button next to the Wrapper Account URL field. MeedyaDL sends an HTTP GET to the wrapper URL with a 5-second timeout:

- **Success** — shows "Connected" with the round-trip latency in milliseconds (e.g., "Connected (42ms)")
- **Timeout** — "Connection timed out (5s)" — the wrapper may not be running or the URL is wrong
- **Connection refused** — "Connection refused — is the wrapper running at {url}?" — the host is reachable but nothing is listening on that port
- **Other errors** — the specific error message is shown

### Automatic pre-flight check

Every time the download queue starts processing, MeedyaDL runs automatic health checks for internet connectivity, cookies, and (if the wrapper is enabled) the wrapper service. If the wrapper is unreachable, a **yellow toast notification** appears with the specific error message (e.g., "Wrapper service at `http://127.0.0.1:30020` timed out — check that it is running").

This check is **advisory** — downloads will still be attempted, but they may fail if the wrapper is genuinely down. Wrapper notifications are **deduplicated** (only one is shown at a time) and **auto-dismiss** when the wrapper becomes reachable again on a subsequent download.

### Troubleshooting Wrapper Connectivity

If MeedyaDL reports the wrapper is unreachable (yellow toast or "Test Connection" failure), work through the steps below from a terminal on the **machine running MeedyaDL**.

#### Step 1 — Verify the URL in MeedyaDL

Open **Settings > Advanced** and check the **Wrapper Account URL**. It should look like:

- Local: `http://127.0.0.1:30020` (wrapper on the same machine)
- Remote: `http://192.168.x.x:30020` (wrapper on another device, e.g. a Raspberry Pi)

Make sure the IP address and port are correct. If the wrapper is on another device, use that device's LAN IP — not `127.0.0.1`.

#### Step 2 — Test from your terminal with curl

Open a terminal on the machine running MeedyaDL and run:

```
curl -v http://192.168.x.x:30020
```

Replace the URL with your actual wrapper URL. Possible outcomes:

- **A response (any HTTP status)** — the wrapper is running and reachable. If MeedyaDL still fails, double-check the URL matches exactly.
- **"Connection refused"** — the host is reachable but nothing is listening on that port. The wrapper process may not be running, or it's on a different port.
- **"Connection timed out" / no response** — the host is not reachable. Check network, firewall, or IP address.
- **"Could not resolve host"** — the hostname/IP is wrong. Verify the address.

#### Step 3 — Check the wrapper is running on the host

SSH into the wrapper host (or open a terminal locally) and check:

**Docker:**
```
docker ps | grep wrapper
```
If no output, the container isn't running. Start it with `docker start <container_name>` or `docker compose up -d`.

Check container logs for errors:
```
docker logs <container_name> --tail 50
```

**Native (systemd):**
```
systemctl status wrapper
```

**Native (manual):**
```
ps aux | grep wrapper
```

If the process isn't running, start it according to the wrapper's own documentation.

#### Step 4 — Check the port is open (remote setups)

If the wrapper is on a different machine, the port must be accessible over the network.

**On the wrapper host**, check the port is listening:
```
ss -tlnp | grep 30020
```
or:
```
netstat -tlnp | grep 30020
```

You should see the wrapper listening on `0.0.0.0:30020` (all interfaces) or your LAN IP. If it only shows `127.0.0.1:30020`, the wrapper is only accepting local connections — you'll need to configure it to bind to `0.0.0.0` or the LAN interface.

**Docker port mapping:** ensure the container maps the port to the host. Check with:
```
docker port <container_name>
```
You should see `30020/tcp -> 0.0.0.0:30020`. If not, recreate the container with `-p 30020:30020`.

#### Step 5 — Check firewall rules

**Linux (ufw):**
```
sudo ufw status
sudo ufw allow 30020/tcp
```

**Linux (iptables):**
```
sudo iptables -L -n | grep 30020
```

**macOS:** Check System Settings > Network > Firewall (or `/usr/libexec/ApplicationFirewall/socketfilterfw --listapps`).

**Windows:** Check Windows Defender Firewall > Inbound Rules for port 30020.

**Router/NAT:** If the wrapper is behind a router on a different subnet, you may need port forwarding. For devices on the same LAN, this is usually not needed.

#### Step 6 — Verify from MeedyaDL

After resolving the issue, go back to **Settings > Advanced** and click **Test Connection**. You should see "Connected" with a latency reading. If it still fails, repeat from Step 2.

---

## Cookie authentication versus wrapper

| Feature | Cookie Auth | Wrapper |
|---|---|---|
| Setup difficulty | Easy (browser extension export) | Advanced (local server) |
| Dolby Atmos access | Sometimes unreliable | More reliable |
| Session duration | Cookies expire periodically | Persistent while server runs |
| Dependencies | None (cookies file only) | Wrapper service |
| Platform support | All platforms | Linux x86_64 (native) or remote/Docker |
| Recommended for | Most users | Advanced users needing reliable Atmos access |

---

## Automatically retrying without the wrapper

When a wrapper download fails (all retries and fallbacks exhausted), MeedyaDL normally shows a **"Retry without Wrapper"** button on the failed queue item. Clicking it re-queues the download with wrapper disabled, falling back to cookie-based authentication.

If you'd prefer this to happen **automatically**, enable **Auto-Retry without Wrapper** in **Settings > Advanced > Wrapper**. When enabled:

1. A wrapper download fails terminally (all retries exhausted)
2. MeedyaDL automatically re-queues the item with wrapper disabled
3. The Activity Log shows "Wrapper failed — auto-retrying without wrapper"
4. The download retries with cookie-based authentication — no manual intervention needed

This is particularly useful if your wrapper service is intermittently unavailable and you want downloads to proceed regardless.

---

## Settings reference

| Setting | Location | Default |
|---|---|---|
| Use Wrapper | Settings > Advanced | Off |
| Auto-Retry without Wrapper | Settings > Advanced | Off |
| Wrapper Account URL | Settings > Advanced | `http://127.0.0.1:30020` |

---

## GAMDL ↔ wrapper-v2 version lockstep

GAMDL and wrapper-v2 are separate projects and must be kept on compatible versions:

- **GAMDL 3.0 – 3.5.x** → wrapper-v1 (three local sockets).
- **GAMDL 3.6 – 3.8.1** → wrapper-v2 **0.0.1** (HTTP decrypt).
- **GAMDL 3.8.2 – 3.8.5** → wrapper-v2 **0.0.2** (native **TCP** decrypt on a separate port, default `10020`). Supported. (3.8.2 introduced the 0.0.2 protocol; 3.8.3–3.8.5 keep it unchanged — 3.8.4 also fixed a wrapper-decrypt bug that could corrupt the end of some songs.)

GAMDL and wrapper-v2 must be upgraded **in lockstep** — GAMDL 3.8.2+ exact-matches wrapper-v2's reported version `0.0.2` at startup and aborts otherwise, and GAMDL ≤ 3.8.1 still uses the old HTTP `POST /decrypt` endpoint that 0.0.2 removed (→ 404). If you run GAMDL 3.8.2+ with a **remote/LAN** wrapper-v2, you MUST also set the **Wrapper decryption IP** (Settings → Advanced → Wrapper) to the daemon's `host:10020` — MeedyaDL now passes it to GAMDL as `--wrapper-decrypt-host`/`--wrapper-decrypt-port`. If MeedyaDL reports *"GAMDL and the wrapper-v2 daemon must be upgraded together"*, your GAMDL and wrapper-v2 versions have drifted apart.
