<!--
  MeedyaDL Help Documentation
  Copyright (c) 2026 MeedyaDL
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Troubleshooting

This guide covers common errors you may encounter while using MeedyaDL, along with their solutions and guidance on finding and interpreting log files.

---

## Overview

MeedyaDL classifies errors into the following categories: **auth**, **network**, **codec**, **not_found**, **rate_limit**, **tool**, and **unknown**. When an error occurs, the application identifies which category it belongs to and displays an appropriate message with guidance. If MeedyaDL is not working as expected, start by identifying your problem in the common errors section below, then consult the log files for more detailed diagnostic information if needed.

---

## Common Errors and Solutions

### Authentication / Cookie Errors (auth)

#### "Authentication failed"

Your Apple Music cookies have expired or are invalid. This is the most common error and typically happens when your browser session with Apple Music has ended since you last exported cookies.

- **Cause:** The cookies that MeedyaDL uses to authenticate with Apple Music are expired, revoked, or were not exported correctly.
- **Solution:** Re-export your cookies from your browser and re-import them into MeedyaDL. Open your browser, sign in to Apple Music if needed, export the cookies, then go to **Settings > Cookies** in MeedyaDL and import the new cookie file. See [Cookie Management](cookie-management.md) for step-by-step instructions.

#### "Cookie file not found"

The cookie file that MeedyaDL is configured to use does not exist at the expected path. This can happen if the file was moved, deleted, or if the path was entered incorrectly.

- **Cause:** The cookie file path stored in MeedyaDL's settings points to a file that no longer exists or is inaccessible.
- **Solution:** Re-import your cookie file via **Settings > Cookies** tab. This will update the stored path to the correct location. If you need to export cookies again, see [Cookie Management](cookie-management.md).

---

### Pre-Download Connectivity Check

MeedyaDL checks your internet connection before every download. If you are offline when you click "Add to Queue", the download is still added to the queue but **will not start processing** until connectivity returns. A yellow warning toast appears: **"Download queued — will start when internet is available."**

Once you are back online, adding another download or clicking "Start Queue" on the Queue page will trigger the queue to process all waiting items. Cookies are not checked when offline since the check would fail anyway.

**Note:** Downloads that ultimately fail due to network issues do **not** generate error reports, since the root cause is connectivity rather than an application bug.

---

### Network Errors (network)

Network errors include connection timeouts, DNS resolution failures, and server-side errors from Apple Music. MeedyaDL automatically retries network errors up to **3 times** with exponential backoff before reporting a failure, so if you see a network error, it means multiple attempts have already been made.

#### Connection Timeout / DNS Failure

- **Cause:** Your internet connection is down, unstable, or a DNS server is unreachable.
- **Solution:** Check your internet connection. Try loading `https://music.apple.com` in your browser to verify connectivity to Apple's servers. If your connection is working but the error persists, try again in a few minutes as Apple's servers may be experiencing temporary issues.

#### Server Errors (HTTP 5xx)

- **Cause:** Apple Music's servers are experiencing problems or undergoing maintenance.
- **Solution:** Wait a few minutes and try again. You can check [Apple's System Status page](https://www.apple.com/support/systemstatus/) to see if Apple Music is experiencing a known outage.

#### Firewall and Proxy Configuration

If you are behind a corporate firewall or use a proxy, MeedyaDL needs to be able to reach Apple Music's servers. The application respects the system proxy settings on all platforms. If you are using a VPN, ensure it does not interfere with connections to Apple's content delivery servers.

---

### Codec / Quality Errors (codec)

#### "Requested quality not available"

Not all content on Apple Music is available in every codec and resolution. Some tracks may only be available in specific formats.

- **Cause:** The specific codec or quality level you requested is not available for this particular content on Apple Music.
- **Solution:** Enable fallback quality in **Settings > Fallback** tab so that MeedyaDL automatically selects the next best available quality when your preferred choice is unavailable. Alternatively, manually select a different quality level before downloading. See [Fallback Quality](fallback-quality.md) for configuration details and [Quality Settings](quality-settings.md) for an overview of available formats.

---

### Not Found Errors (not_found)

#### Storefront Mismatch (different country in URL vs your account)

If you paste a URL from a country other than your Apple Music account region (e.g. a `/us/` link while your account is `gb`), and the album either isn't published in the URL's catalog or your account can't license it from that storefront, MeedyaDL will see an `AMP API Status code: 404 Resource Not Found` error.

- **Default behaviour (v0.52+):** MeedyaDL automatically retries the download once using your account region (Settings > General > Storefront). You will see an activity-log line such as `Storefront 'us' returned no catalog entry — retrying with your account region 'gb'…`. If the album exists in your region, the retry succeeds and the file lands without further action.
- **If both storefronts fail:** the activity log shows `Album not available in 'us' or 'gb'` and the item moves to the failed bucket. The album may genuinely be region-locked or removed from the catalog.
- **To disable the auto-retry:** uncheck **Settings > General > Auto-retry with your region when a URL's storefront fails**. Strict URL-storefront behaviour is restored.
- **Why the auto-retry is one-shot only:** to prevent two URLs whose albums are mutually unavailable from ping-ponging forever. A manual click on the **Retry** button refreshes the budget, so you can ask for the rewrite again.

#### Content Not Found

- **Cause:** The content has been removed from Apple Music, the URL is invalid or malformed, or the content is not available in your configured region.
- **Solution:** Verify that the URL is correct by opening it directly in your browser at `https://music.apple.com`. If the content no longer appears on Apple Music, it has been removed by the rights holder and cannot be downloaded.

---

### Rate Limit Errors (rate_limit)

#### Too Many Requests

Apple Music limits the number of requests that can be made in a given time period. If you are downloading many items simultaneously, you may hit this limit.

- **Cause:** Too many requests have been sent to Apple Music's servers in a short period of time.
- **Solution:** Reduce the number of concurrent downloads in **Settings > General** tab. Wait a few minutes before retrying, as the rate limit will reset automatically. If you are downloading a large playlist or discography, consider reducing concurrency to 2-3 simultaneous downloads to avoid triggering rate limits.

---

### Tool Errors (tool)

#### Missing Dependencies

MeedyaDL relies on external tools such as **FFmpeg** and **mp4decrypt** to process downloaded content. If these tools are missing or corrupted, you will see a tool error.

- **Cause:** A required dependency (FFmpeg, mp4decrypt, or another tool) is not installed, is not on the system PATH, or has become corrupted.
- **Solution:** Go to **Settings > Advanced > Re-run Setup** to re-download and install all required dependencies automatically. This will verify and repair the dependency installation without affecting your other settings.

---

### Wrapper Errors

#### Wrapper Service Unreachable

If you use wrapper authentication and MeedyaDL reports the wrapper is unreachable (yellow toast notification), check that the wrapper service is running and that the URL in **Settings > Advanced** is correct. See the in-app help topic **Help > Wrapper > Troubleshooting Wrapper Connectivity** for detailed diagnostic steps.

**Notification behaviour:** Wrapper warnings appear as persistent yellow toast notifications that auto-dismiss when the wrapper becomes reachable again on a subsequent download. Identical notifications are never stacked — only one wrapper warning is shown at a time.

#### Auto-Retry without Wrapper

If wrapper downloads fail frequently, you can enable **Auto-Retry without Wrapper** in **Settings > Advanced > Wrapper**. When enabled, failed wrapper downloads are automatically re-queued with wrapper disabled (falls back to cookie-based authentication). This saves you from manually clicking "Retry without Wrapper" on each failed item.

Without this setting enabled, failed wrapper downloads show a **"Retry without Wrapper"** button on the queue item, allowing you to manually retry with cookie-based auth.

#### "Decryption is not available" warnings (wrapper enabled, downloads still skipping)

This is a GAMDL warning that means: *"the wrapper accepted the connection, but it couldn't decrypt this track."* You'll see lines like this in the Activity Log:

```text
[Track 14/14] Skipping "Just Getting Started.": Decryption is not available for media ID: 1021848727
```

The most common cause when the wrapper is enabled is **stale wrapper authentication**. The wrapper service logs into Apple Music once with your credentials and keeps the resulting tokens in its data directory. Those tokens can expire over time — often after a few weeks or after Apple invalidates the session for any reason. When that happens, the wrapper still *appears* to be working: MeedyaDL's pre-download checks for the account / m3u8 / decryption services all pass (the sockets accept connections), the download starts, and the manifest fetch succeeds — but every per-track decryption request fails because the wrapper's stored Apple Music tokens are no longer valid.

##### How to re-authenticate the wrapper

You re-run the wrapper's one-off **login command** with your Apple Music username and password. The wrapper saves fresh tokens to its data directory; subsequent runs use those tokens normally.

**If you're running the wrapper via Docker** (the most common setup):

1. Stop the running wrapper container:

   ```sh
   docker compose down
   ```

2. Run the wrapper in **login mode** with your Apple credentials:

   ```sh
   docker run --privileged --rm -it \
     -v ./rootfs/data:/app/rootfs/data \
     --entrypoint ./wrapper \
     ghcr.io/worldobservationlog/wrapper:local \
     -L "your.apple.id@example.com:your-apple-password" \
     -H 0.0.0.0
   ```

   The wrapper will sign into Apple Music. If your Apple ID has two-factor authentication, it will prompt for the 6-digit code — enter it and let the login finish.

3. Once login completes (you'll see confirmation in the terminal), press **Ctrl-C** to quit.

4. Start the wrapper normally again:

   ```sh
   docker compose up -d
   ```

**If you're running the wrapper natively** (not via Docker), the equivalent is to run the wrapper binary with the `-L "username:password"` flag once, let it sign in, quit, then start it again normally.

##### How to tell if it's an auth issue vs a different problem

Stale wrapper auth typically shows *every* track in a download skipping with the same "Decryption is not available" message. If only some tracks skip and others succeed, it's more likely that those specific tracks just aren't available in the requested codec — adjust the [Fallback Quality chain](fallback-quality.md) so MeedyaDL retries with a different format (ALAC / AAC) for the skipped ones. The Activity Log entries themselves look identical for both cases; the rate of skips is your diagnostic.

##### Other possible causes

- **You haven't enabled wrapper auth at all.** Atmos, AC-3 and other experimental codecs are wrapper-only — cookie-based auth alone can't decrypt them. Toggle **Use Wrapper** in Settings > Advanced if you haven't already.
- **The track genuinely isn't available in the requested codec.** Some Apple Music regions have different mastering catalogues; not every track has an Atmos mix. The fallback chain handles this automatically when configured.
- **The track was withdrawn or replaced.** Apple periodically replaces individual tracks (e.g., a song migrated from one album release to another). Re-fetching the album page in your browser may reveal it's been re-listed under a different ID.

##### Does changing **Download Mode** help?

No — Download Mode (yt-dlp vs N_m3u8DL-RE in Settings > Advanced) only changes the tool that downloads the encrypted HLS streams from Apple's CDN. It doesn't affect authentication or decryption. If decryption is failing, switching Download Mode won't fix it. (Some users see downloads "start working again" after switching modes because changing the setting forces a fresh attempt with reset state — not because of the mode itself.)

#### Running the wrapper on a different device on your network

You don't have to run the wrapper on the same computer as MeedyaDL. A common setup is to run MeedyaDL on a Mac or Windows PC and the wrapper on a separate Linux box — for example, a Raspberry Pi sitting on the same home network.

This works, but the wrapper actually uses **three** connections — not just one — and MeedyaDL needs to know about all three. If you only update one or two of them, downloads will appear to start successfully but then fail partway through.

In **Settings > Advanced > Wrapper**, you'll find three address fields. Update *all three* to point at the device running the wrapper:

| Setting                          | What it does                          | Default                    |
| -------------------------------- | ------------------------------------- | -------------------------- |
| **Wrapper Account URL**          | The wrapper's account / login service | `http://127.0.0.1:30020`   |
| **Wrapper m3u8 Address**         | The wrapper's playlist service        | `127.0.0.1:20020`          |
| **Wrapper Decryption Address**   | The wrapper's decryption service      | `127.0.0.1:10020`          |

If your wrapper runs on a Raspberry Pi at `192.168.1.50`, change them to:

- **Wrapper Account URL:** `http://192.168.1.50:30020`
- **Wrapper m3u8 Address:** `192.168.1.50:20020`
- **Wrapper Decryption Address:** `192.168.1.50:10020`

Note that the host (`192.168.1.50`) is the same in all three — only the port number differs.

**MeedyaDL will warn you if anything's unreachable.** Before each download starts, MeedyaDL quietly checks all three services and shows a yellow warning toast if any of them can't be reached. The warning tells you which service failed and at what address, so you can fix the right setting.

##### Common gotchas when using a remote wrapper

- **The wrapper service needs to accept network connections, not just local ones.** By default, many wrapper installs only listen on `127.0.0.1` (the device itself). If you've installed it via Docker, that usually means the container needs port-forwarding configured (the `ports:` section in your `compose.yaml`); if you've installed it natively, the wrapper's own configuration needs to listen on `0.0.0.0` instead of `127.0.0.1`.
- **A firewall on the wrapper's device may be blocking the ports.** On Linux, run something like `sudo ufw allow 30020/tcp` (and the same for `20020` and `10020`).
- **Forgetting to update all three settings.** If the Account URL points at the Raspberry Pi but the Decryption Address still says `127.0.0.1`, MeedyaDL will fetch metadata fine but downloads silently fail at the decryption stage. The pre-download warnings catch this.
- **You need three free ports on the same device.** The wrapper can't share its three ports with anything else on its host, but the host *can* run other services on different ports.

##### Quick alternative: SSH tunnel

If you'd rather not change MeedyaDL's settings at all, you can forward all three ports through SSH from your MeedyaDL machine. From a terminal on the Mac/PC running MeedyaDL:

```sh
ssh -L 10020:localhost:10020 -L 20020:localhost:20020 -L 30020:localhost:30020 user@your-pi-address
```

While the SSH session stays open, the three ports on the Mac/PC tunnel through to the same ports on the Raspberry Pi. MeedyaDL can keep its default `127.0.0.1` settings and everything just works. Useful for trying it out before committing to the address-change approach.

---

### Retrying Failed or Partial Downloads

#### Per-item retry

- **Queue page:** every failed item has a circular-arrow **Retry** button on the right of its row. Wrapper-using items also expose **Retry without Wrapper**. Right-click a row for the same actions plus **Copy Source Link** and **Open Folder**.
- **History page:** every failed entry now has a **Retry** button (rotate-ccw icon) and the same actions are available via right-click. The original history entry is preserved when you retry; the new download writes its own entry on completion.

#### Bulk retry: "Retry All Failed"

The header on both **Queue** and **History** pages shows a **Retry All Failed (N)** button when at least one failed item exists. A confirmation modal lists the count before re-queueing. On the History page, duplicate URLs are deduplicated automatically (12 failed entries for the same URL → 1 re-enqueue).

#### Smart retry: only the failed tracks re-run

When you retry an item that previously produced a partial download (e.g. `GAMDL reported N per-track error(s)`), MeedyaDL reads the `manifest.meedyadl` file in the album folder, diffs the expected track list against the audio files actually on disk, and re-runs GAMDL with `album_url?i={song_id}` URLs covering only the missing tracks. Behaviour:

- If **every track** is already on disk, the retry is refused with `Nothing to retry — all N track(s) are already on disk` instead of pointlessly re-queueing.
- If **some tracks** are missing, only those are re-fetched. A 50-track box set with 3 failed tracks runs in seconds, not minutes.
- If **no manifest** is found (the previous run failed before enrichment finished), the retry falls back to the original behaviour: re-running the full album URL with `overwrite=false` so existing files are kept.
- The Activity Log shows the smart-retry decision: `Smart retry for {id}: targeting 3 of 50 track(s) — 3 URL(s) queued`.

---

### Application Errors

#### MeedyaDL Won't Launch

##### macOS

macOS Gatekeeper blocks applications that are not signed with an Apple Developer certificate. Since MeedyaDL is not distributed through the Mac App Store, you may need to explicitly allow it.

- **Solution:**
  1. Right-click (or Control-click) the MeedyaDL app and select **Open** from the context menu.
  2. In the dialog that appears, click **Open** to confirm.
  3. If that does not work, go to **System Settings > Privacy & Security**, scroll down, and click **Open Anyway** next to the MeedyaDL message.
  4. You may need to repeat this process twice on the first launch.

##### Windows

Windows SmartScreen may block the installer or the application from running because it is not recognized.

- **Solution:**
  1. When the SmartScreen dialog appears, click **More info**.
  2. Click **Run anyway** to proceed.
  3. If the installer fails to run or install correctly, try downloading the installer again. If the issue persists, check that your system meets the minimum requirements (Windows 10 or later).

##### Linux

MeedyaDL requires certain system libraries that may not be installed by default on all Linux distributions.

- **Solution:**
  1. Install the required system libraries:

     ```bash
     sudo apt-get install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev
     ```

  2. If you are using the AppImage distribution, make sure it is executable:

     ```bash
     chmod +x MeedyaDL.AppImage
     ```

     Then run it directly: `./MeedyaDL.AppImage`
  3. On non-Debian-based distributions, use your package manager to install the equivalent packages (e.g., `webkit2gtk4.1`, `libappindicator-gtk3`, `librsvg2`).

#### Crash on Launch (Queue Recovery Panic)

The application crashes immediately on startup, and macOS shows a dialog asking whether to "reopen windows".

- **Cause:** A corrupt or incompatible `queue.json` file in the app data directory can trigger a panic during the queue restoration step at startup. This was fixed in v0.5.3, where the queue recovery code was updated to not depend on the Tokio async runtime being available during early app initialisation.
- **Solution:** Update to the latest version of MeedyaDL. If you cannot update immediately, delete the `queue.json` file from your app data directory to allow the application to start:

  | Platform | Path |
  | --- | --- |
  | macOS | `~/Library/Application Support/io.github.meedyadl/queue.json` |
  | Windows | `%APPDATA%/io.github.meedyadl/queue.json` |
  | Linux | `~/.local/share/io.github.meedyadl/queue.json` |

  Deleting this file clears any queued or failed downloads from the previous session but does not affect your settings, cookies, or already-downloaded files.

#### Temp Directory Write Errors

Downloads fail with a permission error related to the temp or working directory.

- **Cause:** On macOS, apps launched from `/Applications` have a working directory of `/`, which is not writable. GAMDL's default temp path of `.` (current directory) fails in this scenario.
- **Solution:** MeedyaDL automatically resolves the temp path to `{OS temp}/MeedyaDL` to avoid this issue. If you still see write errors, verify the temp directory in **Settings > Paths** points to a writable location, or clear it to use the default.

#### FUSE Mount / Cloud Mount Issues

If the app freezes during downloads or shows "Output directory timed out" errors when using cloud storage mounts:

- **Cause:** The output directory points to an unresponsive cloud mount (CloudMounter on macOS, rclone/SSHFS on Linux). File operations on disconnected or slow FUSE mounts can block for minutes. Prior to v0.6.2, this could freeze the entire UI.
- **Solution (v0.6.2+):** MeedyaDL now uses `spawn_blocking` with timeouts for all file I/O operations during the enrichment pipeline, preventing UI freezes. The output path writability check has a 5-second timeout. If your cloud mount is unresponsive:
  1. Check that the cloud mount is connected and responsive
  2. Consider using a local output directory and moving files to cloud storage after download
  3. If the mount is permanently disconnected, change the output directory in **Settings > General > Output**

#### GAMDL Backend Not Found

The embedded Python environment or the GAMDL package itself is corrupted, incomplete, or missing from the expected location.

- **Cause:** The bundled Python installation or the GAMDL package has been corrupted, was not installed correctly during initial setup, or was accidentally deleted.
- **Solution:** Go to **Settings > Advanced > Re-run Setup**. This will re-download and install both the embedded Python environment and the GAMDL package from scratch without affecting your cookies, settings, or downloaded files.

#### Settings Not Saving

Changes to settings are not persisted between application restarts.

- **Cause:** The application data directory or the settings file within it does not have the correct file permissions, preventing MeedyaDL from writing changes.
- **Solution:** Verify that the app data directory is writable by your user account. The settings file is located at:

  | Platform | Settings Directory |
  | --- | --- |
  | macOS | `~/Library/Application Support/io.github.meedyadl/` |
  | Windows | `%APPDATA%/io.github.meedyadl/` |
  | Linux | `~/.local/share/io.github.meedyadl/` |

  If fixing permissions does not help, try deleting the `settings.json` file in that directory to reset all settings to their defaults. MeedyaDL will recreate the file on next launch.

---

### Quality and Format Errors

#### Output File Won't Play

After downloading, the file does not play in your media player.

- **Cause:** Your media player does not support the codec or container format of the downloaded file. This is especially common with lossless (ALAC) or high-resolution formats.
- **Solution:** Use [VLC](https://www.videolan.org/vlc/), which supports virtually all audio and video codecs. If you need files that are compatible with the widest range of players and devices, re-download the content in **AAC** format, which is the most universally supported audio format.

#### Some Tracks Skipped / Partial Album Download

Only some tracks in an album were downloaded, with the rest showing "Requested format is not available" in the Activity Log.

- **Cause:** When downloading with Dolby Atmos or AC-3 as the preferred codec *without* a wrapper, these experimental formats may not be available for every track on the album. GAMDL skips tracks where the format is unavailable instead of falling back per-track.
- **What MeedyaDL does:** MeedyaDL automatically detects partial downloads and re-runs the download with non-experimental codecs (e.g., ALAC, AAC) and `overwrite` disabled. This fills in the missing tracks without overwriting the successfully downloaded Atmos/AC-3 files. You'll see "Gap-fill complete" in the Activity Log when this succeeds.
- **If gap-fill also fails:** Enable a wrapper in **Settings > Advanced** to allow experimental codecs to fall back correctly for all tracks, or switch to a non-experimental preferred codec like **ALAC** or **AAC**.

---

### Download Output Issues

#### "No output files" but GAMDL Succeeded

The download appears to complete (GAMDL exits successfully) but MeedyaDL reports "no output files" or marks the item as failed.

- **Cause:** In earlier versions, MeedyaDL's output detection could miss files when GAMDL used certain naming patterns or when the output directory contained unexpected characters. This was a known issue in versions prior to v0.10.
- **Solution:** Update to the latest version of MeedyaDL. This issue has been fixed in recent releases with improved output file detection logic. If you are already on the latest version and still see this error, check that your output directory path does not contain special characters, and verify the directory is writable.

#### Activity Log Shows Authentication Method

When a download starts, the Activity Log now displays which authentication method is being used: **"Downloading via wrapper at {url}"** or **"Downloading with cookie-based authentication"**. This is normal informational output, not an error.

- **Purpose:** This helps you confirm which authentication path is active for each download, which is useful when troubleshooting failures. If a download fails with an auth error and the log shows cookie-based authentication, the most likely fix is to refresh your cookies. If the log shows wrapper authentication, check that the wrapper service is running and reachable.
- **Note:** When the **Verbose Activity Log** is enabled in Settings > Advanced, additional authentication details are shown, including wrapper URL, credential status, and token expiry information.

#### Activity Log Auto-Scroll

The Activity Log auto-scrolls to the bottom by default. An **Auto-scroll** checkbox in the toolbar shows the current state:

- **Checked (default):** The log scrolls to the bottom as new entries arrive.
- **Scrolling up:** The checkbox automatically unchecks, freezing the view so you can read earlier entries without losing your place.
- **Re-checking:** Jumps back to the bottom and resumes auto-scrolling.

The Activity Log retains up to **10,000 entries** per session. When the limit is reached, the oldest entries are trimmed. Trimmed entries are not lost — they are persisted to the on-disk activity log (see below) so post-hoc forensic analysis remains possible.

---

### Activity Log Export

The Activity Log toolbar offers **three** actions for preserving log data, each suited to a different scenario:

| Button | What it exports | When to use it |
| --- | --- | --- |
| **Export** | The entries currently visible in the Activity Log panel — respects any active search or category filter. | Quick share of a specific slice of the log. Matches what you see on screen. |
| **Export Disk** | The persistent on-disk activity log — the complete, untrimmed record of the last three days. | Bug reports and post-hoc forensic analysis. Captures events that were trimmed from the 10,000-line in-memory view and every verbose line regardless of filter state. |
| **Reveal** | Opens the logs folder in Finder / Explorer / your file manager. | Attaching raw log files to a GitHub issue, archiving them manually, or pointing a support channel at them. |

#### How to Export the Visible Activity Log

1. Open the **Activity Log** panel (accessible from the sidebar or the bottom panel).
2. (Optional) Narrow the view with the search box or the System / Download / Verbose category toggles.
3. Click the **Export** button in the Activity Log header.
4. Choose a save location in the native file dialog — the default filename includes a timestamp (e.g., `MeedyaDL-activity-log_2026-04-22_14h05m.log`).
5. The exported `.log` file is plain-text with one entry per line, each prefixed with a timestamp. System events are marked with `[System]` and download events include the download ID.

To export the complete unfiltered in-memory view, clear the search field and enable all category filters (System, Download, Verbose) before exporting.

#### How to Export the On-Disk Activity Log

MeedyaDL writes every activity log event to a daily-rotating file on disk (`activity-YYYY-MM-DD.log`) as it happens. This is the authoritative record for bug hunting because it includes:

- Entries from earlier in the session that were trimmed from the 10,000-line in-memory buffer.
- Every verbose event, even when the Verbose filter in the UI is off.
- Events written before the Activity Log panel was ever opened in that session.

To export it, click **Export Disk** in the Activity Log toolbar. MeedyaDL concatenates the last three daily log files, prepends a header line with each file's date, and opens a native save dialog. A success toast shows how many KB were written.

You can also browse the log files directly by clicking **Reveal** — this opens the logs folder in your OS file manager. Files are named `activity-YYYY-MM-DD.log`; you can attach them directly to bug reports.

> **Retention:** On-disk activity logs are kept for **7 days** before being pruned automatically, matching the retention window for the tracing log. A heavy download session typically writes under 5 MB per day.

#### Changing where on-disk activity logs are stored

By default, activity logs live in the same `logs/` directory as the tracing and session logs (see *Log File Locations* below). You can point them at a different directory — for example, an external drive — via **Settings > Advanced > Diagnostics > On-disk activity log location**. Click **Browse…** to select a folder, or paste a path directly into the field. Click **Reset** to return to the default. The change applies on the next app restart. If the chosen path is not writable at startup, MeedyaDL falls back to the default and logs a warning.

---

## Log Files

### Log File Locations

MeedyaDL writes three kinds of daily-rotating log files to its `logs/` subdirectory under the platform app data directory:

| Filename pattern | Contents | Retention |
| --- | --- | --- |
| `meedyadl.YYYY-MM-DD.log` | `tracing` structured log output from the Rust backend. | 7 days |
| `activity-YYYY-MM-DD.log` | Persistent on-disk activity log — every event emitted to the Activity Log panel, including events that were trimmed from the in-memory buffer and every verbose line regardless of UI filter state. Written by a dedicated background writer with buffered I/O so it never blocks downloads. | 7 days |
| `session-YYYY-MM-DD.log` | Older fallback: entries that were trimmed out of the 10,000-line Activity Log as a session overflowed. Largely superseded by `activity-*.log`, but kept for backwards compatibility. | 30 days |

The default `logs/` directory:

| Platform | Log File Location |
| --- | --- |
| macOS | `~/Library/Application Support/io.github.meedyadl/logs/` |
| Windows | `%APPDATA%/io.github.meedyadl/logs/` |
| Linux | `~/.local/share/io.github.meedyadl/logs/` |

You can override the location of `activity-*.log` files via **Settings > Advanced > Diagnostics > On-disk activity log location** — useful for pointing logs at an external drive. The `meedyadl.*` tracing logs and `session-*` files always live in the default directory.

### Reading Log Files

Log entries are prefixed with a timestamp, log level, and module name. The log levels indicate the severity of each message:

| Level | Meaning |
| --- | --- |
| **ERROR** | Something failed. An operation could not be completed. These entries are the most important to look at when diagnosing problems. |
| **WARN** | A potential issue was detected, but the operation may still have succeeded. Worth reviewing if something seems wrong. |
| **INFO** | Normal operational messages. These confirm that the application is working as expected (e.g., download started, download completed). |
| **DEBUG** | Detailed diagnostic information intended for developers and advanced troubleshooting. Only visible when verbose logging is enabled. |

When diagnosing a problem, search the log file for **ERROR** entries first. The timestamp on the error entry will help you correlate it with the specific download attempt that failed. Look at the lines immediately before the error for additional context about what the application was doing when the failure occurred.

### Verbose Activity Log

MeedyaDL includes a **Verbose Activity Log** setting in **Settings > Advanced** that shows detailed diagnostic information directly in the Activity Log panel.

> **Session-only setting:** As a safety measure, verbose logging automatically resets to **off** every time MeedyaDL is restarted. This prevents sensitive data (authentication tokens, cookie paths, API responses, MusicKit credentials) from being logged permanently by accident. You will need to re-enable it each session if needed.

When enabled, you will see:

- **Codec detection results**: ffprobe-detected codec vs. requested codec, and the resolved effective codec used for tagging
- **Enrichment parameters**: requested codec, native priority mode, output directory
- **Suffix decisions**: Why files get or don't get codec suffixes (e.g., native priority uses clean filenames)
- **JWT claims**: Team ID, Key ID, and token expiry used for Apple Music API authentication
- **API parse results**: Album name, track count, artwork availability, UPC
- **API response dump**: The raw Apple Music API response is saved as `<AlbumName>-applemusic-data.json` in the album output directory. Useful for verifying API integration after endpoint changes.

This is the recommended first step for troubleshooting metadata, codec tagging, or API issues.

### Enabling Verbose Logging

By default, MeedyaDL logs at the **INFO** level. To capture more detailed diagnostic information, set the `RUST_LOG` environment variable to `debug` before launching the application:

**macOS / Linux (Terminal):**

```bash
RUST_LOG=debug /path/to/MeedyaDL
```

**Windows (Command Prompt):**

```batch
set RUST_LOG=debug
MeedyaDL.exe
```

**Windows (PowerShell):**

```powershell
$env:RUST_LOG="debug"
.\MeedyaDL.exe
```

Verbose logging produces significantly more output and may cause log files to grow quickly. Only enable it when actively troubleshooting an issue, and remember to disable it afterward by launching the application normally without the environment variable.

> **Note:** The `RUST_LOG` environment variable controls the *log file* verbosity and is independent of the in-app **Verbose Activity Log** toggle (which controls the Activity Log panel). Both reset when the application is restarted — `RUST_LOG` because it is an environment variable, and the in-app toggle because it is a session-only setting.

---

## Crash Reports

When MeedyaDL encounters a crash (Rust panic) or an unhandled frontend error, it automatically saves a JSON crash report to the crashes directory. These reports contain the error message, stack trace, app version, and platform information.

### Crash Report Locations

| Platform | Crash Report Directory |
| --- | --- |
| macOS | `~/Library/Application Support/io.github.meedyadl/crashes/` |
| Windows | `%APPDATA%/io.github.meedyadl/crashes/` |
| Linux | `~/.local/share/io.github.meedyadl/crashes/` |

Crash reports are named `crash-YYYYMMDD-HHMMSS.json` and are automatically cleaned up after 30 days.

### Anonymous Crash Reporting (Optional)

You can optionally help improve MeedyaDL by enabling anonymous crash reporting in **Settings > Advanced > Crash Reporting**. When enabled, crash data (error message, stack trace, app version, OS) is sent to our error tracking service. No personal data, download history, or account information is ever included. This feature is disabled by default and requires explicit opt-in.

### Reporting a Crash via GitHub Issues

You can report crashes directly to the developer from within MeedyaDL. This opens a pre-filled GitHub Issue in your browser -- no API tokens or server accounts needed, just a GitHub account.

1. Go to **Settings > Advanced > Crash Reporting**.
2. Recent crash reports are listed below the Sentry toggle. Each entry shows the date, time, and error summary.
3. Click **Report** next to the crash report you want to submit.
4. A preview dialog appears showing exactly what data will be included: error message, backtrace, app version, operating system, and timestamp. Review the information to ensure you are comfortable sharing it.
5. Click **Open GitHub Issue** to open a pre-filled issue in your default web browser.
6. On GitHub, add any additional context such as the steps you took before the crash occurred, then click **Submit new issue**.

**Notes:**

- A [GitHub account](https://github.com/signup) is required to submit issues.
- No personal data, download history, cookies, or account information is included in the report.
- If the crash report's backtrace is very long, it will be automatically truncated to fit within URL length limits.
- You can also **Delete** crash reports you no longer need from the same list.

---

## Reporting a Bug

If you encounter a problem that is not covered in this guide, or if the suggested solutions do not resolve your issue, please report it as a bug:

1. **Note the app version.** You can find this in **Settings > About** or in the application title bar.
2. **Copy relevant log entries.** Open the log file (see [Log File Locations](#log-file-locations) above) and copy the ERROR entries along with the surrounding context lines. If possible, enable verbose logging, reproduce the issue, and include the debug-level log entries.
3. **Note the steps to reproduce.** Write down exactly what you did that triggered the error, including the URL you were trying to download, the quality settings you had selected, and any other relevant configuration.
4. **Open an issue on the GitHub repository.** Include the app version, your operating system and version, the log entries, and the reproduction steps. The more detail you provide, the faster the issue can be diagnosed and resolved.

---

## Related Topics

- [Cookie Management](cookie-management.md) -- Resolving authentication and cookie issues
- [Quality Settings](quality-settings.md) -- Understanding quality and format options
- [Fallback Quality](fallback-quality.md) -- Configuring quality fallback behavior
- [FAQ](faq.md) -- Frequently asked questions
- [Getting Started](getting-started.md) -- Initial setup and configuration

---

[Back to Help Index](index.md)
