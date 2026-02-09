<!--
  gamdl-GUI Help Documentation
  Copyright (c) 2024 MWBM Partners Ltd
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :cookie: Cookie Management

This guide explains how to export your Apple Music cookies from a web browser, import them into gamdl-GUI, and troubleshoot common cookie-related issues.

---

## Overview

gamdl-GUI uses your Apple Music browser cookies to authenticate with Apple's servers on your behalf. These cookies prove that you have a valid Apple Music subscription, allowing gamdl-GUI to access and download content. Cookies are essential for the application to function -- without valid cookies, downloads will fail.

---

## Why Cookies Are Needed

> *Explanation of why cookie-based authentication is used.*

Placeholder for details on:

- How Apple Music authentication works
- Why cookies are the method used for authentication
- What data the cookies contain (session tokens, not your password)
- Privacy considerations and how cookies are stored locally

---

## Exporting Cookies from Your Browser

### Prerequisites

- You must be signed in to [music.apple.com](https://music.apple.com) in your browser
- You must have an active Apple Music subscription
- A browser extension or developer tool for exporting cookies

### Using a Browser Extension

> *Step-by-step instructions for exporting cookies using a browser extension.*

Placeholder for detailed instructions covering:

1. Installing a cookie export extension (e.g., "Get cookies.txt" or similar)
2. Navigating to music.apple.com
3. Activating the extension to export cookies
4. Saving the cookie file in the correct format

### From Chrome

> *Chrome-specific cookie export instructions.*

Placeholder for Chrome-specific steps, including any relevant developer tools methods.

### From Firefox

> *Firefox-specific cookie export instructions.*

Placeholder for Firefox-specific steps, including any relevant developer tools methods.

### From Safari

> *Safari-specific cookie export instructions.*

Placeholder for Safari-specific steps, noting any macOS-specific considerations.

### From Edge

> *Microsoft Edge-specific cookie export instructions.*

Placeholder for Edge-specific steps.

---

## Importing Cookies into gamdl-GUI

### Using the Import Dialog

> *How to import your exported cookie file into gamdl-GUI.*

Placeholder for step-by-step instructions:

1. Open gamdl-GUI
2. Navigate to the cookie settings (describe where this is in the UI)
3. Click the import button
4. Select your exported cookie file
5. Verify that the cookies were imported successfully

### Cookie File Formats

> *Details on the cookie file formats accepted by gamdl-GUI.*

Placeholder for information on supported formats (e.g., Netscape cookie format, JSON, etc.) and any format conversion that may be necessary.

### Verifying Cookie Validity

> *How to check whether your imported cookies are still valid.*

Placeholder for details on:

- The cookie validation indicator in gamdl-GUI
- How to test cookies without starting a full download
- What a valid vs. expired cookie status looks like in the UI

---

## Cookie Expiry and Renewal

### How Long Do Cookies Last?

> *Information on cookie expiration timelines.*

Placeholder for details on:

- Typical Apple Music cookie lifespan
- Factors that can cause cookies to expire early (e.g., signing out of Apple Music in the browser, changing your Apple ID password)
- Warning signs that your cookies are about to expire

### Renewing Expired Cookies

> *Step-by-step process for replacing expired cookies.*

Placeholder for instructions on:

1. Recognizing that your cookies have expired (error messages, failed downloads)
2. Signing back into music.apple.com in your browser
3. Re-exporting cookies using the same process described above
4. Re-importing the fresh cookies into gamdl-GUI

---

## Troubleshooting Cookie Issues

### Common Error Messages

> *Cookie-related error messages and what they mean.*

Placeholder for a list of common error messages related to cookies, such as:

- "Authentication failed" -- cookies are invalid or expired
- "Cookie file not found" -- the cookie file path is incorrect
- "Invalid cookie format" -- the cookie file is in an unsupported format

### Cookies Expire Quickly

> *What to do if your cookies keep expiring sooner than expected.*

Placeholder for troubleshooting steps when cookies have a very short lifespan.

### Downloads Fail Despite Valid Cookies

> *Troubleshooting downloads that fail even when cookies appear valid.*

Placeholder for steps to diagnose issues where cookies seem valid but downloads still fail. See also [Troubleshooting](troubleshooting.md) for general error resolution.

---

## Security and Privacy

### How Cookies Are Stored

> *Details on how gamdl-GUI stores your cookies locally.*

Placeholder for information on:

- Where cookie data is stored on disk
- Whether cookies are encrypted at rest
- How to manually delete stored cookies

### Best Practices

> *Recommendations for keeping your cookies and account secure.*

Placeholder for security best practices such as:

- Never sharing your cookie file with others
- Storing cookie files securely
- Periodically refreshing cookies
- What to do if you suspect your cookies have been compromised

---

## Related Topics

- [Getting Started](getting-started.md) -- Initial cookie setup during first-time configuration
- [Downloading Music](downloading-music.md) -- How cookies are used during music downloads
- [Downloading Videos](downloading-videos.md) -- How cookies are used during video downloads
- [Troubleshooting](troubleshooting.md) -- General error resolution and log file locations

---

[Back to Help Index](index.md)
