<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaDL
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

---

## Log Files

### Log File Locations

MeedyaDL writes log files to the application data directory on each platform:

| Platform | Log File Location |
| --- | --- |
| macOS | `~/Library/Application Support/io.github.meedyadl/logs/` |
| Windows | `%APPDATA%/io.github.meedyadl/logs/` |
| Linux | `~/.local/share/io.github.meedyadl/logs/` |

### Reading Log Files

Log entries are prefixed with a timestamp, log level, and module name. The log levels indicate the severity of each message:

| Level | Meaning |
| --- | --- |
| **ERROR** | Something failed. An operation could not be completed. These entries are the most important to look at when diagnosing problems. |
| **WARN** | A potential issue was detected, but the operation may still have succeeded. Worth reviewing if something seems wrong. |
| **INFO** | Normal operational messages. These confirm that the application is working as expected (e.g., download started, download completed). |
| **DEBUG** | Detailed diagnostic information intended for developers and advanced troubleshooting. Only visible when verbose logging is enabled. |

When diagnosing a problem, search the log file for **ERROR** entries first. The timestamp on the error entry will help you correlate it with the specific download attempt that failed. Look at the lines immediately before the error for additional context about what the application was doing when the failure occurred.

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
