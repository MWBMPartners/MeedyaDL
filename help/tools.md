<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# External Tools

MeedyaDL uses a few small helper programs to do its work. You can check them, install them and update them from **Settings > Tools** at any time. The setup wizard installs them for you the first time.

---

## The Five Helper Tools

| Tool | What it is for |
|---|---|
| **FFmpeg** | Handles sound and video files, and joins the parts of a video together. Needed for most downloads. |
| **mp4decrypt** | Used by older versions of GAMDL. |
| **N_m3u8DL-RE** | Used for downloads only if you change **Download Mode** in Settings > Advanced. |
| **MP4Box** | Used by older versions of GAMDL. |
| **MediaInfo** | Reads a finished file to tell what quality it really is, so MeedyaDL can show you what you got. |

On newer GAMDL versions some of these tools are no longer used, but keep them installed. FFmpeg and MediaInfo are also used by MeedyaDL itself.

<details>
<summary>More detail: which tools newer GAMDL versions still need</summary>

From **GAMDL 3.6**, GAMDL no longer needs MP4Box or mp4decrypt for music videos. FFmpeg and MediaInfo are still needed on every GAMDL version. FFmpeg is also used by MeedyaDL itself for its loudness (ReplayGain) check, and MediaInfo helps MeedyaDL spot the real format of a finished file.

</details>

---

## Installing and Managing Tools

After the first set-up, go to **Settings > Tools** to:

- **Check All.** Refresh the status of every tool.
- **Install missing tools.** One at a time, or all at once with **Install All (N missing)**.
- **Reinstall.** Every installed tool has a **Reinstall** button. It asks you first, then downloads the tool again or uses a suitable copy already on your computer.
- **Use your own copy.** Open a tool with the arrow on its right and choose a copy you installed yourself, for example one you installed for your whole computer.

If a later MeedyaDL update adds a new tool, the Tools tab shows it as missing so you can install it.

<details>
<summary>More detail: Reinstall and package managers</summary>

If your copy came from a package manager (for example Homebrew or APT), Reinstall may ask that package manager to update it instead. That can ask for your password, and MeedyaDL asks you first. If your current copy works and the new one does not, your current copy is kept.

</details>

---

## Trying GAMDL 4

GAMDL 4 is a rewrite of GAMDL. MeedyaDL can use it, but it has **not been tested on real downloads yet**. For that reason it is not installed unless you ask for it. MeedyaDL keeps recommending GAMDL 3.9.1.

**How to try it.**

1. Go to **Settings > Tools > GAMDL Version Management**.
2. Type `4.0.2` (or a newer GAMDL 4 release) under **Advanced: install specific version** and press **Install**.
3. To go back, press **Install recommended** in the same place.

While you are on GAMDL 3, the **Updates** page offers GAMDL 3 updates only, so nobody is moved to GAMDL 4 by accident. Once you are on GAMDL 4, it offers GAMDL 4 updates, marked **Untested**.

**Use 4.0.2, not 4.0.0 or 4.0.1.** GAMDL 4.0.0 cannot save downloads to drives that do not support "hard links". That includes most portable drives and many network or cloud folders, so downloads to them fail. GAMDL 4.0.0 and 4.0.1 can also save some short songs in a way that means they will not play, while still reporting them as downloaded. 4.0.2 fixes all of this, and MeedyaDL will not install 4.0.0 or 4.0.1. If you already have one of them, **Settings > Tools** marks it with a known problem. Install 4.0.2 from the **Updates** page or from the same **Advanced: install specific version** box.

**Not every computer can use it.** On Linux on 32-bit ARM (for example an older Raspberry Pi), GAMDL 4 has no package to install. On Windows on ARM, MeedyaDL stays on GAMDL 3.8.5 for now. On both, MeedyaDL will not install GAMDL 4 and tells you why.

**Your settings carry over.** Nothing needs converting. A few settings work differently, or not at all, with GAMDL 4, and **Settings** says so beside each one.

<details>
<summary>More detail: what changes with GAMDL 4</summary>

- **Cover art.** From GAMDL 4.0.2, **Save Cover Art** works as it does with GAMDL 3: one album picture is saved in the album's folder. With GAMDL 4.0.0 and 4.0.1 the **Save Cover Art** switch has no effect, because they would save a picture beside every song. MeedyaDL saves one album picture itself instead, when it can get the album's details from Apple Music. **Cover Size** sets the size of the picture inside each song, as with GAMDL 3. The "original file" format choice gives a JPEG picture at that size.
- **Synced Lyrics Only.** GAMDL 4 cannot download lyrics on their own, so downloads will not start while this is on. Turn it off to download as usual.
- **Download Mode** and **Remux Mode** (Settings > Advanced) are not used. GAMDL 4 handles those steps itself.
- **Unlocking Method** (Settings > Advanced) is not used with GAMDL 4 yet. The default is used instead, and the Activity Log says so.
- **mp4decrypt, MP4Box and N_m3u8DL-RE** are not used for Apple Music downloads with GAMDL 4. Keep them installed if you might go back to GAMDL 3. MeedyaDL itself still uses FFmpeg and MediaInfo.
- **Your download history is safe.** GAMDL 4 can keep its own list of what it has downloaded and skip anything on it. MeedyaDL switches that list off, so downloading an album again, or filling in a missing format, still works.

</details>

If something goes wrong with GAMDL 4, please [report it](troubleshooting.md#reporting-a-bug) and go back to 3.9.1 meanwhile.

---

## Keeping the tools up to date

When MeedyaDL checks for updates, it checks all five tools too. It lists what it finds on the **Updates** page. Each tool is compared with the place a new copy would really come from, so you are never offered a "new" version from somewhere MeedyaDL would not install it from.

**Copies you installed yourself are left alone.** If MeedyaDL is using a copy that a package manager put on your computer (for example Homebrew or apt), or one it simply found there, it does not check it or update it. Update that copy the same way you installed it.

**"Could not check" is shown, not hidden.** Sometimes MeedyaDL cannot tell whether a tool is up to date. This can be because:

- there is no internet connection,
- the tool will not start, so it cannot report its version,
- nobody recorded who installed that copy, or
- the source does not publish a version MeedyaDL can compare.

The Updates page then lists the tool under "could not check" with the reason, and the heading says **No updates found** rather than **You're up to date!**. Usually you do not need to do anything.

- If the reason says MeedyaDL **could not run** the tool, it also shows as missing in **Settings > Tools**, with an **Install** button.
- If the reason says the tool **started but reported something other than a version** (usually an error), it still shows as installed. Press its **Reinstall** button in **Settings > Tools** to replace it.

**Your working copy is protected.** If your current copy works, MeedyaDL keeps it until the new copy is fully in place and has started and reported its version. If the new copy cannot be downloaded or does not work on your computer, MeedyaDL puts your previous copy back and tells you. This is true for **Update**, **Install** and **Reinstall** alike. If your current copy does not work either, there is nothing safe to go back to, so the new copy is used. The one thing MeedyaDL cannot undo is a change a package manager makes on its own, such as a Homebrew update. If that copy stops starting afterwards, MeedyaDL says so instead of reporting success.

<details>
<summary>More detail: where each tool's updates come from</summary>

- **N_m3u8DL-RE** is compared with its own releases on GitHub.
- **FFmpeg** is compared with the same place your copy was installed from. Builds are compared by the date they were made, because many have no ordinary version number. An update is offered only when the newer build is more than 30 days newer than yours, because new builds appear very often.
- **mp4decrypt** and **MediaInfo** are compared with the MeedyaSuite mirror, a copy of the tools kept by the MeedyaDL project.
- **MP4Box** from the MeedyaSuite mirror is compared with the mirror. A copy from GPAC's own installer is compared with the version MeedyaDL is set up to install. At the moment none is set, so it shows as "could not check".

Pressing **Upgrade** on the Updates page never swaps in a copy it happens to find elsewhere on your computer. It falls back to the MeedyaSuite mirror only when the mirror is what the check compared against. (FFmpeg may still come from its usual download site rather than the mirror. That copy is the same age or newer.)

"Works" means the copy starts and, within 30 seconds, reports a version MeedyaDL recognises. For FFmpeg it also covers ffprobe, the part MeedyaDL uses to look inside downloaded files. If your current copy's ffprobe works, the new copy's must too.

</details>
