<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Wrapper authentication

The wrapper is an optional extra program. It lets MeedyaDL sign in to Apple Music without your cookies file. **Most people do not need it.** Cookies are enough for almost everything.

You run the wrapper yourself, either on this computer or on another one. MeedyaDL does not include it. This page tells you when it is worth the trouble, how to point MeedyaDL at it, how to check that it works, and what to do when it does not.

---

## Do I need a wrapper?

Probably not. Without a wrapper you can still download songs, albums, playlists, lyrics, artwork and most audio formats. What you can get depends on the GAMDL version MeedyaDL has installed. You can see it under **Settings > Tools**.

- **GAMDL 3.8 or newer:** the only thing that still needs a wrapper is **ALAC** (lossless). Everything else works with cookies, including Dolby Atmos and Dolby Digital.
- **GAMDL 3.0 to 3.7:** you need a wrapper for ALAC, Dolby Atmos, Dolby Digital and the other AAC formats. AAC Legacy and AAC-HE Legacy still work with cookies.

If you are happy with the standard lossy AAC quality, skip the wrapper. If you do not want to use a wrapper, see [Cookie Management](cookie-management.md).

---

## Set it up

1. Install and start the wrapper by following that project's own instructions. There are two kinds, and which one you need depends on your GAMDL version:

   | Your GAMDL version | Wrapper to use |
   |---|---|
   | 3.0 to 3.5 | [wrapper-v1](https://github.com/WorldObservationLog/wrapper) |
   | 3.6 and newer | [wrapper-v2](https://github.com/glomatico/wrapper-v2) |

   On Windows and macOS, wrapper-v2 needs Docker. On a Mac you can use Docker Desktop or [OrbStack](https://orbstack.dev/). On Linux it can run directly.
2. In MeedyaDL, open **Settings > Advanced > Wrapper**.
3. Turn on **Use Wrapper**.
4. Check the address fields (see the next section). The defaults are right if the wrapper runs on this computer with its default settings.
5. With the newer wrapper (GAMDL 3.6 and newer), press **Sign in to wrapper** and sign in with your Apple ID. MeedyaDL does not store your password. Next to the button you will see **Signed in**, **Not signed in** or **Wrapper unreachable**.

> **Why signing in matters:** with the newer wrapper, a download started while you are signed out would wait for a sign-in that never comes. MeedyaDL checks before each download and shows a yellow message telling you to sign in first.

### The address fields

Which fields you see depends on your GAMDL version. On GAMDL 3.0 the m3u8 field is shown but not used.

| GAMDL version | Field | Default |
|---|---|---|
| 3.6 and newer | **Wrapper URL** | `http://127.0.0.1` |
| 3.0 to 3.5 | **Wrapper Account URL** | `http://127.0.0.1:30020` |
| 3.1 to 3.5 | **Wrapper m3u8 Address** | `127.0.0.1:20020` |
| 3.0 to 3.5 | **Wrapper Decryption Address** | `127.0.0.1:10020` |

`127.0.0.1` means "this computer". If your wrapper runs on another computer, put that computer's address in its place.

<details>
<summary>More detail: which GAMDL version to pick</summary>

MeedyaDL supports GAMDL 3.x only. If you still have GAMDL 2.x, MeedyaDL marks it as unsupported and downloads may fail. Upgrade it in **Settings > Tools**.

- **You use the older wrapper (wrapper-v1):** upgrade to GAMDL 3.5. It is the last release that works with that wrapper. Going past 3.5 means switching to the newer wrapper.
- **You do not use a wrapper:** upgrade to the version MeedyaDL recommends. It is the newest one that has been tested.
- Two kinds of computer cannot use the newest releases: Windows on ARM stays on 3.8.5, and 32-bit Raspberry Pi (ARMv7) stays on 3.8.1. MeedyaDL will not offer an update that cannot install.
- **GAMDL 3.9.0 is refused.** It cannot download the AAC Legacy formats. MeedyaDL will not install it. Version 3.9.1 fixes the problem.

The older wrapper will not be supported for ever. A later MeedyaDL release may require the newer wrapper for the wrapper-only formats. If you rely on the older one today, plan to switch.

</details>

<details>
<summary>More detail: keeping GAMDL and the newer wrapper in step</summary>

GAMDL and the newer wrapper must be on matching versions:

- **GAMDL 3.6 to 3.8.1** works with wrapper-v2 version 0.0.1.
- **GAMDL 3.8.2 and newer** works with wrapper-v2 version 0.0.2.

If they do not match, MeedyaDL says *"GAMDL and the wrapper-v2 daemon must be upgraded together"*. Update whichever one is behind.

On GAMDL 3.8.2 and newer, there is one more address, the decryption address. It matters only when the wrapper runs on a different computer. The default is `127.0.0.1:10020`. Settings does not show this address with the newer layout. To change it, edit `wrapper_decrypt_ip` in `settings.json` in the app data folder, and use the other computer's address with port `10020`.

</details>

---

## Run the wrapper on another computer

You can run the wrapper on an always-on machine, such as a home server or a Raspberry Pi, and share it with several computers.

1. Set up the wrapper on that machine and start it.
2. Check that another computer on the same network can reach it. Open its address in a browser, or run `curl http://<its-address>` in a terminal.
3. In MeedyaDL, put that machine's address in the address fields instead of `127.0.0.1`.
4. With the older wrapper, **change all three addresses**, not just the first one. This is the most common reason a remote wrapper "does not work". If you change only the first, signing in seems to work and then the download fails partway through.
5. With the newer wrapper, press **Sign in to wrapper** again.

You can also edit these addresses in `settings.json` in the app data folder (`use_wrapper`, `wrapper_account_url`, `wrapper_m3u8_ip`, `wrapper_decrypt_ip`). The Settings screen is easier.

### Keep it on a network you trust

**The newer wrapper has no password of its own.** Anyone who can reach its address can use your Apple Music sign-in through it.

- That is fine on this computer alone (`127.0.0.1`) and on a home network you control.
- It is risky on shared networks (cafe, hostel, dorm, shared office) and on any address reachable from the internet.

When your address is not on this computer, MeedyaDL shows a note under the field. An **amber** note means a home-network or name-based address. A **red** note means a public address, which is almost always a mistake.

<details>
<summary>More detail: making a remote wrapper safer</summary>

- **Firewall it.** Allow only your own computers to reach the wrapper's port, and block everyone else.
- **Use an SSH tunnel.** Keep the wrapper on its own computer's loopback address, and open a tunnel from your computer. Then point MeedyaDL at the tunnel's local address.
- **Use a separate network segment** (a VLAN) with only your own computers on it.
- **Use a VPN** such as Tailscale or WireGuard, and do not expose the wrapper to the internet at all.

On a single trusted home network you do not need any of these.

</details>

---

## Check that it works

**Test Connection.** In **Settings > Advanced > Wrapper**, press **Test Connection**. It tries the **Wrapper Account URL** address and waits up to 5 seconds:

- **Connected (42ms)** (the number will differ): the wrapper answered.
- **Connection timed out (5s):** the wrapper may not be running, or the address is wrong.
- **Connection refused:** the computer answered but nothing is listening. Is the wrapper running?

The test only talks to addresses on your own computer or your own network. Any other address is refused without being contacted. With the newer wrapper, the **Signed in** / **Wrapper unreachable** line next to **Sign in to wrapper** is the better check.

**Automatic check.** Each time the queue starts, MeedyaDL checks that the wrapper answers. If it does not, a yellow message appears. The check is only a warning. Downloads are still tried. The warning goes away by itself once the wrapper answers again. Whether your computer also shows it as a system notification follows **Settings > General > Notifications**.

---

## If something goes wrong

**"Wrapper ... unreachable", or every download shows a yellow message.**

1. Check the address. Wrapper on this computer: `http://127.0.0.1`, or `http://127.0.0.1:30020` for the older one. Wrapper on another computer: use that computer's network address, never `127.0.0.1`.
2. Check the wrapper is running. If you use Docker, run `docker ps` and look for it. If it is missing, start it again with `docker compose up -d`. `docker logs <container>` shows its recent messages.
3. From the computer running MeedyaDL, run `curl -v <address>`. Any answer at all means the wrapper is reachable. "Connection refused" means nothing is listening. "Timed out" means a network or firewall problem. "Could not resolve host" means the name is wrong.

**"Reachable but not signed in", on every download.** Press **Sign in to wrapper** in **Settings > Advanced > Wrapper**.

**It says unreachable on a Mac even though Docker shows it running.** Docker on a Mac sometimes does not reconnect cleanly after a restart. Try `http://localhost` instead. Or run `docker compose down` and then `docker compose up -d`.

**It works from the same computer but not from MeedyaDL on another one.** Usually a firewall on the wrapper's computer is blocking connections from other computers. Test from a third computer on the network. If that also fails, the wrapper is only listening on its own computer, or the firewall is blocking it.

**Downloads stall for a long time with "Companion: downloading aac ...".** Your GAMDL version needs the wrapper for that format, and the wrapper is not set up. Either finish setting it up, or choose a companion mode that does not need it.

**A yellow or amber note about your network address appears in Settings.** That is the safety reminder above. If the address is deliberate, you can ignore it.

<details>
<summary>More detail: checking ports and firewalls on the wrapper's computer</summary>

If the wrapper is on another computer, its port must be open to the network.

- **Listening address.** On the wrapper's computer, run `ss -tlnp | grep <port>` (or `netstat -tlnp | grep <port>`). You want to see it listening on all addresses (`0.0.0.0`) or its network address. If you only see `127.0.0.1`, it accepts connections from its own computer only.
- **Docker.** Run `docker port <container>` and check the port is mapped to the host. If not, recreate the container with the port published.
- **Linux firewall.** `sudo ufw status`, then `sudo ufw allow <port>/tcp`.
- **macOS.** Check **System Settings > Network > Firewall**.
- **Windows.** Check **Windows Defender Firewall > Inbound Rules** for the port.
- **Different network.** If the wrapper is behind a router on a different subnet you may need port forwarding. On one home network you normally do not.

After fixing it, press **Test Connection** again.

</details>

---

## Wrapper or cookies?

| | Cookies | Wrapper |
|---|---|---|
| Setup | Easy | Advanced |
| Lasts | Cookies expire from time to time | Stays signed in while it runs |
| Needs | A cookies file | A separate program running |
| Best for | Most people | People who want lossless ALAC, or older GAMDL users who want Atmos |

On the Settings screen you may see a note that the wrapper only has ready-made programs for Linux x86_64. On other computers, run it on another machine or in Docker and point MeedyaDL at it.

---

## Retry without the wrapper

When a wrapper download fails after every retry, the failed item shows a **Retry without Wrapper** button. It re-queues the item using your cookies instead.

To do that automatically, turn on **Auto-Retry without Wrapper** in **Settings > Advanced > Wrapper** (it appears once **Use Wrapper** is on). The Activity Log then shows "Wrapper failed — auto-retrying without wrapper". It is handy if your wrapper is sometimes offline.

---

## Settings at a glance

| Setting | Where | Default | Which GAMDL |
|---|---|---|---|
| Use Wrapper | Settings > Advanced > Wrapper | Off | All |
| Auto-Retry without Wrapper | Settings > Advanced > Wrapper | Off | All |
| Wrapper URL | Settings > Advanced > Wrapper | `http://127.0.0.1` | 3.6 and newer |
| Wrapper Account URL | Settings > Advanced > Wrapper | `http://127.0.0.1:30020` | 3.0 to 3.5 |
| Wrapper m3u8 Address | Settings > Advanced > Wrapper | `127.0.0.1:20020` | 3.1 to 3.5 |
| Wrapper Decryption Address | Settings > Advanced > Wrapper | `127.0.0.1:10020` | 3.0 to 3.5 |

See also [Settings](settings.md) and [Troubleshooting](troubleshooting.md).

[Back to Help Index](index.md)
