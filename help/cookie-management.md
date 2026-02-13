<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaDL
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :cookie: Cookie Management

This guide explains how to set up Apple Music authentication in MeedyaDL using cookies, including the built-in login window, browser auto-import, manual cookie export, and troubleshooting common cookie-related issues.

---

## Overview

MeedyaDL uses your Apple Music browser cookies to authenticate with Apple's servers on your behalf. These cookies prove that you have a valid Apple Music subscription, allowing MeedyaDL to access and download content. Cookies are essential for the application to function -- without valid cookies, downloads will fail.

MeedyaDL provides **three ways** to set up cookies, listed from easiest to most manual:

1. **Built-in Apple Music login** -- Sign in directly within the app (no browser extension needed)
2. **Browser auto-import** -- Automatically extract cookies from an installed browser
3. **Manual cookie file import** -- Export cookies from your browser using an extension and import the file

---

## Built-in Apple Music Login (Recommended)

The easiest way to set up cookies is to sign in to Apple Music directly within MeedyaDL. The app opens an embedded browser window where you sign in with your Apple ID, and cookies are extracted automatically.

### How to Use

1. Open MeedyaDL.
2. Navigate to **Settings > Cookies** tab.
3. Click **"Sign in to Apple Music"**.
4. A login window opens showing the Apple Music sign-in page.
5. Sign in with your Apple ID and password. Complete any two-factor authentication prompts if required.
6. Once signed in successfully, MeedyaDL automatically detects the `media-user-token` cookie (which confirms authentication) and extracts all cookies from the webview.
7. The cookies are saved in Netscape format and the login window closes automatically.

This method requires no browser extensions and works on all platforms. The login window uses Tauri's native webview, which is isolated from your system browsers.

---

## Browser Auto-Import

If you are already signed in to Apple Music in a browser on your computer, MeedyaDL can detect installed browsers and extract your Apple Music cookies automatically.

### Steps

1. Open MeedyaDL.
2. Navigate to **Settings > Cookies** tab.
3. Click **"Import from Browser"**.
4. MeedyaDL scans for installed browsers and displays a list of detected browsers.
5. Select the browser you are signed in to Apple Music with.
6. MeedyaDL extracts the Apple Music cookies and saves them automatically.

### Supported Browsers

- **Chrome** / **Chromium**
- **Firefox**
- **Edge**
- **Safari** (macOS only)
- **Brave**
- **Opera**
- **Vivaldi**

### Requirements

- You must be **signed in to music.apple.com** in the selected browser before importing.
- The browser must be **closed** during import on some platforms (the app will notify you if this is required).
- On **macOS**, importing from Safari or Chrome may require **Full Disk Access** permission for MeedyaDL. The app will prompt you if this is needed and guide you through enabling it in System Settings.

---

## Why Cookies Are Needed

Apple Music requires authentication to access its content. MeedyaDL uses browser cookies -- specifically session tokens -- to authenticate with Apple's servers. This approach means:

- **Your Apple ID password is never stored or transmitted by the app.** MeedyaDL only uses the session tokens contained in cookies that your browser created when you signed in to Apple Music.
- **Cookies act as proof of your active session.** When you sign in to music.apple.com in your browser, Apple creates session cookies. MeedyaDL uses these same cookies to make requests on your behalf.
- **Cookies are stored locally on your machine.** They are saved as a plain text file in the app's data directory. They are never uploaded or sent to any third-party server.

This is the same authentication method used by other tools that interact with Apple Music, and it avoids the need to handle or store your Apple ID credentials directly.

---

## Manual Cookie Export from Your Browser

If you prefer to export cookies manually (or if the built-in login and browser auto-import are not available), you can use a browser extension to export your cookies and import the file into MeedyaDL.

### Prerequisites

- You must be signed in to [music.apple.com](https://music.apple.com) in your browser
- You must have an active Apple Music subscription
- A browser extension for exporting cookies (see browser-specific instructions below)

### Cookie File Format

MeedyaDL accepts cookies in the **Netscape/Mozilla cookie file format**. This is a plain text, tab-separated format and is the standard format exported by browser cookie extensions. A valid cookie file typically starts with the following header line:

```text
# Netscape HTTP Cookie File
```

Each subsequent line contains a single cookie with tab-separated fields (domain, flag, path, secure, expiry, name, value). You do not need to edit this file manually -- the browser extensions listed below produce files in the correct format automatically.

### Required Domains

Your exported cookie file must include cookies for the following domains:

- `music.apple.com`
- `.apple.com`

MeedyaDL validates these domains on import and will warn you if required domains are missing.

### From Chrome

1. Install the **"Get cookies.txt LOCALLY"** extension from the [Chrome Web Store](https://chromewebstore.google.com).
2. Navigate to [music.apple.com](https://music.apple.com) and sign in with your Apple ID.
3. Once signed in, click the extension icon in the Chrome toolbar.
4. Click **"Export"** to save the file as `cookies.txt`.
5. The exported file will be in Netscape format and saved to your Downloads folder.

### From Firefox

1. Install the **"cookies.txt"** add-on from [Firefox Add-ons](https://addons.mozilla.org).
2. Navigate to [music.apple.com](https://music.apple.com) and sign in with your Apple ID.
3. Once signed in, click the extension icon in the Firefox toolbar.
4. Select **"Current Site"** to export only the cookies for Apple Music.
5. Click export to save the file. The file will be in Netscape format.

### From Safari

Safari does not have a direct cookie export extension equivalent to the Chrome or Firefox options. There are two alternatives:

- **Use a different browser.** The simplest approach is to sign in to music.apple.com in Chrome, Firefox, or Edge, and use one of the extensions listed above to export your cookies.
- **Use Web Inspector.** Open Safari, go to the **Develop** menu > **Show Web Inspector** > **Storage** > **Cookies**. From here you can view cookies, but you would need to manually assemble them into Netscape format, which is not recommended.

For the easiest experience, use Chrome or Firefox for cookie export.

### From Edge

1. Install **"Get cookies.txt LOCALLY"** from the [Edge Add-ons store](https://microsoftedge.microsoft.com/addons) (this is the same extension available for Chrome).
2. Navigate to [music.apple.com](https://music.apple.com) and sign in with your Apple ID.
3. Once signed in, click the extension icon in the Edge toolbar.
4. Click **"Export"** to save the file as `cookies.txt`.
5. The process is identical to Chrome.

---

## Importing Cookies into MeedyaDL

### Using the Import Dialog

You can import cookies in two places:

- **During first-run setup:** The setup wizard presents cookie import at **Step 5**. Follow the on-screen prompts.
- **From Settings:** Go to **Settings > Cookies** tab at any time.

To import your cookie file:

1. Open MeedyaDL.
2. Navigate to **Settings > Cookies** tab (or reach Step 5 of the first-run setup wizard).
3. Click the **"Import Cookie File"** button.
4. In the file picker, select your exported `cookies.txt` file.
5. MeedyaDL will automatically:
   - Validate the file format (must be Netscape/Mozilla format).
   - Check for required Apple Music domains (`music.apple.com` and `.apple.com`).
   - Show expiry warnings if any cookies are near expiration.

### Verifying Cookie Validity

After a successful import, the Cookies tab displays a validation summary with the following information:

- **Domain badges** -- Shows which Apple domains are present in the cookie file (e.g., `music.apple.com`, `.apple.com`).
- **Expiry status** -- Indicates whether your cookies are valid, expiring soon, or expired:
  - **Green** -- Cookies are valid and not near expiry.
  - **Yellow** -- Cookies are expiring soon. You should re-export and re-import soon.
  - **Red** -- Cookies have expired. You must re-export and re-import before downloads will work.
- **Cookie count** -- The total number of cookies found in the imported file.
- **File path** -- The location where the cookie file is stored. You can copy this path to your clipboard.

---

## Cookie Expiry and Renewal

### How Long Do Cookies Last?

Apple Music cookies typically last **1 to 12 months**, depending on the specific cookie type. However, cookies may expire earlier than expected if:

- You **sign out** of Apple Music in your browser.
- You **change your Apple ID password**.
- Apple **rotates session tokens** on their end.
- The cookie is a short-lived session cookie rather than a persistent one.

MeedyaDL monitors cookie expiry dates and warns you when cookies are approaching expiration, so you can proactively refresh them before downloads start failing.

### Renewing Expired Cookies

When your cookies expire or are approaching expiry, use any of these methods to refresh them:

- **Use the built-in login** -- Go to **Settings > Cookies** and click **"Sign in to Apple Music"**. Sign in again and fresh cookies are extracted automatically.
- **Use browser auto-import** -- If you are still signed in to music.apple.com in a browser, click **"Import from Browser"** to re-extract fresh cookies.
- **Manual re-export** -- Re-export your cookies from your browser using the same extension you used before, then re-import the file in MeedyaDL.

The new cookies replace the old ones. Verify that the status indicator shows green after renewal.

---

## Troubleshooting Cookie Issues

### Common Error Messages

| Error Message | Cause | Solution |
| --- | --- | --- |
| **"Authentication failed"** | Cookies are expired or invalid. | Sign in to music.apple.com in your browser, re-export your cookies, and re-import them into MeedyaDL. |
| **"Cookie file not found"** | The cookie file was moved, renamed, or deleted from disk. | Re-import your cookie file from **Settings > Cookies** tab. If you no longer have the file, re-export from your browser. |
| **"Invalid cookie format"** | The file is not in Netscape/Mozilla cookie format. | Make sure you are using a supported browser extension (see above). The file should begin with `# Netscape HTTP Cookie File`. Do not edit the file manually. |

### Cookies Expire Quickly

If your cookies seem to expire much sooner than expected:

- **Check that you are not signing out** of Apple Music in your browser after exporting. Signing out can invalidate session cookies immediately.
- **Avoid changing your Apple ID password** unless necessary, as this will invalidate existing sessions.
- **Re-export from a fresh sign-in session.** Close all browser windows, open a new window, sign in to music.apple.com, and export cookies. This ensures you get the freshest tokens.
- **Use a persistent sign-in.** When signing in to music.apple.com, look for a "Keep me signed in" or "Remember me" option if available.

### Downloads Fail Despite Valid Cookies

If the cookie status shows green but downloads still fail:

- **Re-export and re-import** your cookies as a first step -- the status indicator may not catch all edge cases.
- **Check your Apple Music subscription** is still active by visiting music.apple.com in your browser.
- **Check your internet connection** and ensure Apple's servers are reachable.
- **Review the application logs** for more detailed error information.

See also [Troubleshooting](troubleshooting.md) for general error resolution and log file locations.

---

## Security and Privacy

### How Cookies Are Stored

- Cookies are stored as a **plain text file** in the app's private data directory on your machine.
- The exact file path is displayed in **Settings > Cookies** tab, and you can copy it to your clipboard from there.
- Credentials for other services (not Apple Music cookies) use the **OS keychain** for secure storage:
  - **macOS:** Keychain
  - **Windows:** Credential Manager
  - **Linux:** Secret Service

### Best Practices

- **Never share your cookie file with anyone.** Your cookie file grants access to your Apple Music account. Treat it like a password.
- **Do not upload your cookie file** to cloud storage, paste it into chat messages, or include it in bug reports.
- **Refresh cookies proactively.** When MeedyaDL shows a yellow expiry warning, re-export and re-import before they expire completely.
- **If you suspect your cookies have been compromised**, sign out of Apple Music in your browser immediately (this invalidates the session), change your Apple ID password, then sign back in and export fresh cookies.

---

## Related Topics

- [Getting Started](getting-started.md) -- Initial cookie setup during first-time configuration
- [Downloading Music](downloading-music.md) -- How cookies are used during music downloads
- [Downloading Videos](downloading-videos.md) -- How cookies are used during video downloads
- [Troubleshooting](troubleshooting.md) -- General error resolution and log file locations

---

[Back to Help Index](index.md)
