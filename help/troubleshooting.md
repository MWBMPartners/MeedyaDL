<!--
  gamdl-GUI Help Documentation
  Copyright (c) 2024 MWBM Partners Ltd
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :wrench: Troubleshooting

This guide covers common errors you may encounter while using gamdl-GUI, along with their solutions and guidance on finding and interpreting log files.

---

## Overview

If gamdl-GUI is not working as expected, this page will help you diagnose and resolve the issue. Start by identifying your problem in the common errors section below, then consult the log files for more detailed diagnostic information if needed.

---

## Common Errors and Solutions

### Authentication / Cookie Errors

#### "Authentication failed"

> *Cause and resolution for authentication failures.*

Placeholder for details on:

- **Cause:** Your Apple Music cookies are invalid, expired, or missing.
- **Solution:** Re-export and re-import your cookies. See [Cookie Management](cookie-management.md) for detailed instructions.

#### "Cookie file not found"

> *Cause and resolution for missing cookie files.*

Placeholder for details on:

- **Cause:** The cookie file path configured in gamdl-GUI points to a file that no longer exists.
- **Solution:** Re-import your cookie file or verify the file path in settings.

---

### Download Errors

#### "Track not available"

> *Cause and resolution for unavailable track errors.*

Placeholder for details on:

- **Cause:** The requested track may not be available in your region, may have been removed from Apple Music, or may not be available at the requested quality level.
- **Solution:** Try a different quality setting, check regional availability, or verify the URL is correct. See [Fallback Quality](fallback-quality.md) for quality-related options.

#### "Download failed" (generic)

> *Cause and resolution for generic download failures.*

Placeholder for details on:

- **Cause:** Various reasons including network issues, server errors, or disk space problems.
- **Solution:** Check your internet connection, verify available disk space, and consult the log files for specific error details.

#### "Decryption failed"

> *Cause and resolution for decryption errors.*

Placeholder for details on:

- **Cause:** The content could not be decrypted, possibly due to expired cookies or a DRM issue.
- **Solution:** Refresh your cookies (see [Cookie Management](cookie-management.md)) and try again.

---

### Application Errors

#### gamdl-GUI Won't Launch

> *Steps to resolve application startup failures.*

Placeholder for platform-specific troubleshooting:

- **macOS:** Gatekeeper issues, permissions, and how to allow the app in System Settings
- **Windows:** SmartScreen warnings, missing runtime dependencies
- **Linux:** Missing system libraries, permissions issues

#### GAMDL Backend Not Found

> *Cause and resolution when the GAMDL command-line tool cannot be located.*

Placeholder for details on:

- **Cause:** The GAMDL binary is not installed or not in the expected location.
- **Solution:** Verify GAMDL installation and configure the path in gamdl-GUI settings.

#### Settings Not Saving

> *Cause and resolution for settings persistence issues.*

Placeholder for details on:

- **Cause:** File permission issues or corrupted settings store.
- **Solution:** Steps to reset or repair the settings file.

---

### Quality and Format Errors

#### "Requested quality not available"

> *Cause and resolution for quality availability issues.*

Placeholder for details on:

- **Cause:** The specific quality/codec combination requested is not available for this content.
- **Solution:** Enable fallback quality (see [Fallback Quality](fallback-quality.md)) or manually select a different quality (see [Quality Settings](quality-settings.md)).

#### Output File Won't Play

> *Troubleshooting playback issues with downloaded files.*

Placeholder for details on:

- **Cause:** Your media player may not support the codec or container format.
- **Solution:** Try a different media player, or re-download in a more compatible format.

---

## Log Files

### Log File Locations

> *Where to find gamdl-GUI log files on each platform.*

Placeholder for platform-specific log file paths:

| Platform | Log File Location |
|----------|-------------------|
| macOS    | *TBD* |
| Windows  | *TBD* |
| Linux    | *TBD* |

### Reading Log Files

> *How to interpret the information in log files.*

Placeholder for details on:

- Log level meanings (DEBUG, INFO, WARN, ERROR)
- How to find the relevant error in a log file
- Timestamps and how to correlate log entries with download attempts
- What information to include when reporting a bug

### Enabling Verbose Logging

> *How to enable more detailed logging for advanced troubleshooting.*

Placeholder for instructions on enabling debug-level logging to capture more diagnostic information.

---

## Network Issues

### Firewall and Proxy Configuration

> *How to configure gamdl-GUI to work behind firewalls or proxies.*

Placeholder for details on:

- Network ports used by gamdl-GUI
- Proxy configuration options
- VPN compatibility considerations

### Slow Downloads

> *Troubleshooting slow download speeds.*

Placeholder for tips on:

- Checking your internet connection speed
- Reducing simultaneous downloads
- Identifying ISP throttling or network congestion

---

## Reporting a Bug

> *How to report a bug or issue with gamdl-GUI.*

Placeholder for instructions on:

1. Gathering relevant information (application version, OS, log files, steps to reproduce)
2. Where to submit bug reports (GitHub Issues, etc.)
3. What information to include in a bug report
4. How to attach log files

---

## Related Topics

- [Cookie Management](cookie-management.md) -- Resolving authentication and cookie issues
- [Quality Settings](quality-settings.md) -- Understanding quality and format options
- [Fallback Quality](fallback-quality.md) -- Configuring quality fallback behavior
- [FAQ](faq.md) -- Frequently asked questions
- [Getting Started](getting-started.md) -- Initial setup and configuration

---

[Back to Help Index](index.md)
