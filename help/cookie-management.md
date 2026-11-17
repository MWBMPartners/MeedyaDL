<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Cookie Management

Cookies are how MeedyaDL proves to Apple Music that you are signed in with a paid subscription. This page shows how to set them up, how to renew them, and what to do when something goes wrong.

---

## Quick Start

The easiest way:

1. Open **Settings > Cookies**.
2. Click **Sign in with Apple Music**.
3. Sign in with your Apple ID in the window that opens.
4. The window closes by itself when you are done.

That is all. If you would rather use a browser you are already signed in to, see [Browser Auto-Import](#browser-auto-import). If neither works, see [Manual Cookie Export from Your Browser](#manual-cookie-export-from-your-browser).

---

## Overview

When you sign in to Apple Music, your browser keeps a small file called a **cookie** that says "this person is signed in". MeedyaDL uses the same kind of cookie so Apple Music lets it download for you. Without valid cookies, downloads fail.

There are **three ways** to set them up, easiest first:

1. **Built-in Apple Music login.** Sign in inside MeedyaDL. No extras needed.
2. **Browser auto-import.** Copy the sign-in from a browser on your computer.
3. **Manual cookie file.** Export the cookies from your browser with an add-on, then import the file.

---

## Built-in Apple Music Login (Recommended)

MeedyaDL opens its own small browser window. You sign in there, and MeedyaDL saves your cookies for you.

### How to Use

1. Open MeedyaDL.
2. Go to **Settings > Cookies**.
3. Click **Sign in with Apple Music**.
4. A window opens with the Apple Music sign-in page.
5. Sign in with your Apple ID and password. Finish any two-step verification if asked.
6. When MeedyaDL sees that you are signed in, it saves your cookies and closes the window.

This needs no browser add-on and works on every system. The window is separate from your normal browsers, so it does not touch them.

---

## Browser Auto-Import

If you are already signed in to Apple Music in a browser on this computer, MeedyaDL can copy the sign-in for you.

### Steps

1. Open MeedyaDL.
2. Go to **Settings > Cookies**.
3. Under **Import from Browser**, choose your browser from the list.
4. Click **Import Cookies**.
5. MeedyaDL saves your Apple Music cookies.

### Supported Browsers

- **Chrome** and **Chromium**
- **Firefox**
- **Edge**
- **Safari** (macOS only)
- **Brave**
- **Opera**
- **Vivaldi**
- **Arc** (macOS only)

Only browsers MeedyaDL finds on your computer are listed. If none are found, the **Import from Browser** section does not appear.

### Before You Start

- Sign in to **music.apple.com** in that browser first.
- On some systems the browser must be **closed** while you import. MeedyaDL tells you if so.
- On **macOS**, Safari needs **Full Disk Access** for MeedyaDL. The list marks it "requires Full Disk Access". MeedyaDL guides you through turning it on in System Settings.

---

## Why Cookies Are Needed

Apple Music only serves your music to a signed-in subscriber. Cookies are how MeedyaDL shows that you are one.

- **Your Apple ID password is never stored by MeedyaDL.** It only keeps the cookies your sign-in produced.
- **Cookies stay on your computer.** They are saved in a text file in MeedyaDL's data folder. They are never uploaded anywhere.
- **Cookies are a proof of your active sign-in.** Treat them like a password.

---

## Manual Cookie Export from Your Browser

Use this if the built-in login and browser import are not available, or if you prefer to do it yourself. You export your cookies with a browser add-on and then import the file.

### What You Need

- You are signed in to [music.apple.com](https://music.apple.com) in your browser.
- You have an active Apple Music subscription.
- A cookie-export add-on for your browser (see below).

### The File Format

MeedyaDL reads cookies in the **Netscape cookie file** format. It is a plain text file, and the add-ons below produce it for you. You never need to edit it. It normally starts with this line:

```text
# Netscape HTTP Cookie File
```

### Required Cookies

Your file must contain cookies for these two domains:

- `music.apple.com`
- `.apple.com`

MeedyaDL checks for them when you import and warns you if one is missing.

### From Chrome

1. Install **Get cookies.txt LOCALLY** from the [Chrome Web Store](https://chromewebstore.google.com).
2. Go to [music.apple.com](https://music.apple.com) and sign in.
3. Click the add-on icon in the Chrome toolbar.
4. Click **Export** to save `cookies.txt`. It goes to your Downloads folder.

### From Firefox

1. Install the **cookies.txt** add-on from [Firefox Add-ons](https://addons.mozilla.org).
2. Go to [music.apple.com](https://music.apple.com) and sign in.
3. Click the add-on icon in the Firefox toolbar.
4. Choose **Current Site** so only Apple Music's cookies are exported.
5. Click export to save the file.

### From Safari

Safari has no simple cookie-export add-on. Two options:

- **Use another browser.** Sign in to music.apple.com in Chrome, Firefox or Edge and export from there. This is the easiest.
- **Use Safari's Web Inspector.** Open the **Develop** menu > **Show Web Inspector** > **Storage** > **Cookies**. You can see the cookies, but you would have to build the file by hand. We do not recommend that.

### From Edge

1. Install **Get cookies.txt LOCALLY** from the [Edge Add-ons store](https://microsoftedge.microsoft.com/addons). It is the same add-on as for Chrome.
2. Go to [music.apple.com](https://music.apple.com) and sign in.
3. Click the add-on icon in the Edge toolbar.
4. Click **Export** to save `cookies.txt`.

---

## Importing a Cookie File into MeedyaDL

You can do this in two places:

- **In the setup wizard**, at the **Cookies** step (step 5).
- **In Settings**, on the **Cookies** tab, at any time.

To import your file:

1. Open **Settings > Cookies** (or reach the Cookies step in the wizard).
2. Scroll to **Or use a cookies file**.
3. Under **Cookies File**, click Browse and choose your `cookies.txt`.
4. Click **Validate Cookies**.

MeedyaDL then:

- Checks that the file is in the right format.
- Checks that both Apple domains are there.
- Warns you if any cookies are close to running out.

### Checking Your Cookies

After a check, the Cookies tab shows:

- **Domain badges.** Which Apple domains are in the file.
- **Expiry status:**
  - **Green.** Valid, not close to expiring.
  - **Yellow.** Expiring soon. Renew them soon.
  - **Red.** Expired. Renew them before downloading.
- **Cookie count.** How many cookies the file holds.
- **File path.** Where the file is stored. You can copy it with **Copy Cookie Path**.

---

## Cookie Expiry and Renewal

### How Long Do Cookies Last?

Apple Music cookies usually last **1 to 12 months**. They can stop working sooner if:

- You **sign out** of Apple Music in your browser.
- You **change your Apple ID password**.
- Apple signs everyone out on its side.
- A cookie was only meant to last for one browsing session.

MeedyaDL watches the expiry dates and warns you before they run out.

### Renewing Cookies

Use any of these:

- **Built-in login.** Go to **Settings > Cookies** and click **Sign in with Apple Music** again.
- **Browser import.** If you are still signed in to music.apple.com in a browser, choose it under **Import from Browser** and click **Import Cookies**.
- **Manual.** Export the cookies again with the same add-on and import the new file.

New cookies replace the old ones. The status should turn green.

---

## Troubleshooting

### Common Messages

| Message | What it means | What to do |
| --- | --- | --- |
| **"Authentication failed"** | Your cookies have expired or are not valid. | Sign in again with **Sign in with Apple Music**, or re-export and re-import your cookies. |
| **"Cookie file not found"** | The file was moved, renamed or deleted. | Choose the file again on the **Cookies** tab. If you no longer have it, export a new one. |
| **"Invalid cookie format"** | The file is not in the Netscape format. | Use one of the add-ons above. The file should start with `# Netscape HTTP Cookie File`. Do not edit it. |

### Cookies Expire Quickly

- **Do not sign out** of Apple Music in your browser after exporting. That can end the sign-in at once.
- **Avoid changing your Apple ID password** unless you must. It ends existing sign-ins.
- **Export from a fresh sign-in.** Close your browser windows, open a new one, sign in to music.apple.com, then export.
- **Tick "Keep me signed in"** or "Remember me" if you see it.

### Downloads Fail With Green Cookies

- **Renew and re-import** your cookies first. The status light cannot catch every case.
- **Check your subscription** is active by visiting music.apple.com.
- **Check your internet connection.**
- **Look at the Activity page** for details.

See also [Troubleshooting](troubleshooting.md).

---

## Security and Privacy

### Where Cookies Are Kept

- Cookies are saved as a **plain text file** in MeedyaDL's private data folder on your computer.
- **Settings > Cookies** shows the exact path and lets you copy it.
- Sign-in details for other services are kept in your computer's **secure store** instead (macOS Keychain, Windows Credential Manager, Linux Secret Service).

### Good Habits

- **Never share your cookie file.** It gives access to your Apple Music account. Treat it like a password.
- **Do not put it in cloud storage**, chat messages or bug reports.
- **Renew early.** When the status turns yellow, renew before it expires.
- **If you think it has leaked,** sign out of Apple Music in your browser right away, change your Apple ID password, then sign in again and export fresh cookies.

---

## Related Topics

- [Getting Started](getting-started.md) — First-time cookie set-up
- [Downloading Music](downloading-music.md) — How music downloads use your sign-in
- [Downloading Videos](downloading-videos.md) — How video downloads use your sign-in
- [Troubleshooting](troubleshooting.md) — General fixes and where to find logs

---

[Back to Help Index](index.md)
