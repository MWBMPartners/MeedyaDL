<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Animated Artwork

Some albums on Apple Music have **animated cover art**: a short video that loops in place of the still album cover. MeedyaDL can save these videos next to your music. When an album has none, MeedyaDL simply skips this step and shows no error.

---

## What You Get

Each animated cover is saved as a video file in the album folder:

| Kind | File name | Shape | Description |
|------|-----------|-------|-------------|
| **Square** | `FrontCover.mp4` | 1:1 | The usual square cover, animated |
| **Portrait** | `FrontCoverPortrait.mp4` | 3:4 | A tall version made for phones |

If you turn on **Download Artist Promo Video**, you also get `ArtistSpotlightCover.mp4` in the artist's folder (see [Artist Promo Video](#artist-promo-video)).

Not every album has animated art. It is mostly found on newer, popular albums.

**Download Animated Cover Art** and **Download Artist Promo Video** are both on by default. They do nothing until your Apple Music developer details are filled in (see [Setup Guide](#setup-guide)). **Hide Animated Artwork Files** is off by default.

<details>
<summary>More detail: the video files</summary>

- The files are MP4 videos in the H.265 (HEVC) format. Most modern players can play them. Very old software may not.
- Animated art is a property of the whole album. Even if you download one song, MeedyaDL looks for the album's artwork.
- There is no standard way to put an animated cover inside a music file. Music files can only hold still JPEG or PNG pictures. That is why the video is saved as a separate file beside your music.

</details>

---

## What You Need

1. **A free Apple Developer account.** You do **not** need the paid membership.
2. **A MusicKit key** that you create once in the Apple Developer portal.
3. **FFmpeg**, which the setup wizard installs for you.

The setup steps below take about ten minutes. You only do them once.

---

## Setup Guide

### Step 1: Create an Apple Developer Account

If you do not have one, sign up at [developer.apple.com](https://developer.apple.com).

> **Important:** The free account is enough. You do **not** need the paid Apple Developer Program ($99 a year).

1. Go to [developer.apple.com](https://developer.apple.com) and click **Account**.
2. Sign in with your Apple Account (the one you use for iCloud or the App Store).
3. If asked, accept the Apple Developer Agreement.

### Step 2: Create a MusicKit Key

A MusicKit key is a small secret file that tells Apple you are allowed to use its music service from MeedyaDL. You create it once.

1. Sign in to the [Apple Developer Portal](https://developer.apple.com/account).
2. Go to the **Keys** page. Either:
   - In the left sidebar, open **Certificates, Identifiers & Profiles**, then click **Keys**, or
   - Go straight to [developer.apple.com/account/resources/authkeys/list](https://developer.apple.com/account/resources/authkeys/list).
3. Click the **+** button in the top-right corner. If you do not see it, look for **Create a key** or **Register a New Key**.
4. On the **Register a New Key** page:
   - **Key Name:** Type anything, for example "MeedyaDL". It is only a label for you.
   - **Key Services:** Tick the MusicKit option. Free accounts show **MusicKit**. Paid accounts show **Media Services (MusicKit, ShazamKit, Apple Music Feed)**. Either one works.
   - If a **Configure** button appears, click it. You may be asked to pick an **App ID**. Choose any existing one, or make a simple one such as `com.example.meedyadl`. It does not need to be a real app. Click **Save**.
5. Click **Continue**, check the details, then click **Register**.

> **Tip:** If you cannot see any MusicKit option, make sure you are on the "Register a New Key" page. Free accounts sometimes need to accept an updated agreement at [developer.apple.com/account](https://developer.apple.com/account) first.

### Step 3: Download Your Private Key

Apple gives you a key file ending in `.p8`. You can download it **only once**.

1. On the confirmation page, click **Download**. The file is named something like `AuthKey_ABC1234DEF.p8`.
2. Save it somewhere safe, for example a folder called `MeedyaDL Keys` in your Documents.
3. Write down the **Key ID** shown on the page. It is 10 letters and numbers (for example `ABC1234DEF`). You can also find it later on the Keys list.

> **Warning:** If you leave the page without downloading, or lose the file, you cannot download it again. You would have to revoke the key and make a new one (go back to Step 2).

### Step 4: Find Your Team ID

Your Team ID is a 10-character code (for example `ABCDE12345`) that identifies your developer account. You can find it:

- On the **Membership** (or **Membership details**) page in the Apple Developer portal.
- Next to your name in the top-right corner of some portal pages.

### Step 5: Copy the Private Key

The `.p8` file is plain text. You need to copy everything inside it.

1. Open the file in a plain-text editor:
   - **macOS:** TextEdit (choose Format > Make Plain Text first).
   - **Windows:** Notepad (right-click the file > Open with > Notepad).
   - **Linux:** Any text editor.
2. It looks like this:

   ```text
   -----BEGIN PRIVATE KEY-----
   MIGTAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBHkwdwIBAQQg...
   ...several lines of letters and numbers...
   -----END PRIVATE KEY-----
   ```

3. Select everything (`Ctrl+A` or `Cmd+A`), including the **BEGIN** and **END** lines, and copy it.

> **Tip:** Copy the whole thing. Missing even one character makes the key fail.

### Step 6: Enter Your Details in MeedyaDL

1. Open **Settings** > **Cover Art** and check that **Download Animated Cover Art** is on (it is by default).
2. Open **Settings** > **Advanced** > **API Credentials**.
3. Type your Team ID into **MusicKit Team ID**.
4. Type your Key ID into **MusicKit Key ID**.
5. Paste the key you copied into **MusicKit Private Key (.p8)**.
6. Click **Save to Keychain**. MeedyaDL keeps the key in your computer's secure store (macOS Keychain, Windows Credential Manager or Linux Secret Service). The status then reads **Saved to keychain**. Next time you open this screen it reads **Key stored in keychain**.
7. Click **Test Credentials**. MeedyaDL checks your details with Apple and tells you if something is wrong.
8. Click **Save Changes**.

> **If you lost the `.p8` file:** In the Apple Developer portal, open Keys, click your key, click **Revoke**, and start again from Step 2.

---

## How It Works for You

Once the details are saved, nothing more is needed:

1. Download an album as usual.
2. When the album finishes, MeedyaDL checks whether it has animated art.
3. If it does, the video files are saved in the album folder.

This finishes before the next item in the queue starts.

---

## Choosing a Resolution

In **Settings > Cover Art**, **Animated Artwork Resolution** (shown once **Download Animated Cover Art** is on) sets how sharp the video is. It is a ceiling: you get the best quality at or below your choice.

| Option | What it means |
|--------|---------------|
| **Standard (~1080p, recommended)** | Default. Smallest files. Looks the same as the bigger ones at the size artwork is normally shown. |
| **High (~2160p / 4K)** | Larger files, with a difference most people will not notice. |
| **Maximum (highest available, largest files)** | No ceiling. Always the sharpest video available, whatever the size. |

A few extra megabytes per album add up across a big library, so **Standard** is the best choice unless you have a reason to want more.

---

## Artist Promo Video

Some artists have an animated background video on their Apple Music page. With **Download Artist Promo Video** turned on in **Settings > Cover Art**, MeedyaDL:

1. Checks whether the artist has one.
2. Saves it as `ArtistSpotlightCover.mp4` in the **artist folder** (one level above the album folder).
3. Skips it if that file is already there, so it is not downloaded again for every album by the same artist.

> **Note:** Not every artist has one. This needs the same MusicKit details as animated artwork. The option only appears when **Download Animated Cover Art** is on.

---

## Where the Files Go

```text
Taylor Swift/
  ArtistSpotlightCover.mp4           <-- Artist promo video (artist folder)
  Midnights/
    01 Lavender Haze.m4a
    02 Maroon.m4a
    03 Anti-Hero.m4a
    ...
    FrontCover.mp4          <-- Square animated cover
    FrontCoverPortrait.mp4  <-- Portrait animated cover
```

---

## Hiding the Files

**Hide Animated Artwork Files** (in **Settings > Cover Art**, shown when animated art is on) marks the videos as hidden so your album folders show only music. It is **off** by default, so you can see the files.

If you turn it on, the files are hidden after they are saved:

| System | How it is hidden | Keeps its name? | How to see hidden files |
|--------|------------------|-----------------|--------------------------|
| **macOS** | Hidden flag | Yes | In Finder press `Cmd + Shift + .` |
| **Windows** | Hidden flag | Yes | In Explorer choose View > Show > Hidden items |
| **Linux** | A dot is added to the name (`.FrontCover.mp4`) | No | Press `Ctrl + H` in your file manager, or run `ls -a` |

> **Note for Linux:** Hiding renames the file. Software that looks for `FrontCover.mp4` by exact name will not find it. Leave hiding off if you rely on that.

Files saved before you change the setting stay as they were. To show hidden files again, use the last column of the table.

---

## Troubleshooting

### No video files appear

- Check that **MusicKit Team ID**, **MusicKit Key ID** and the private key are all filled in under **Settings > Advanced > API Credentials**.
- The key status should read **Key stored in keychain**, with the note "A private key is stored in your OS keychain…".
- Not every album has animated art. Try a popular, recent album to check that your setup works.

### "Invalid MusicKit private key"

- Copy the **whole** `.p8` file, including the BEGIN and END lines (see [Step 5](#step-5-copy-the-private-key)).
- Do not add spaces or blank lines before or after it.
- Use a plain-text editor. Word processors such as Word or Pages can add invisible characters.
- If you lost the file, revoke the key and make a new one (see [Step 2](#step-2-create-a-musickit-key)).

### Apple says you are not allowed (error 401)

- Your key may have been revoked. Check the Keys page in the Apple Developer portal.
- Your Team ID or Key ID may be wrong. Check them (see [Step 3](#step-3-download-your-private-key) and [Step 4](#step-4-find-your-team-id)).
- Click **Test Credentials** in **Settings > Advanced > API Credentials**. It tells you what went wrong.

### "FFmpeg not installed"

Saving the videos needs FFmpeg. Run the setup wizard, or install it from **Settings > Tools**.

---

## Privacy and Security

- Your **private key** is kept in your computer's secure store. It is never written to settings files or logs.
- Your **Team ID** and **Key ID** are saved in MeedyaDL's settings file. They are not secret.
- Your details go straight from your computer to Apple. They do not pass through any MeedyaDL server.

---

[Back to Help Index](index.md)
