<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Troubleshooting

This page helps you fix common problems in MeedyaDL. Find your problem below and follow the steps. If nothing works, see [Reporting a Bug](#reporting-a-bug) at the end.

---

## Start Here

When something goes wrong, MeedyaDL shows a short message that says what happened and what to do next. The technical text behind the message is folded away under **Details**. On a failed download in the Queue or History, it sits under the short summary. Open **Details** when you report a problem, because that text helps us find the cause.

MeedyaDL sorts problems into a few groups: sign-in, connection, format, not found, too many requests, and helper tools. The sections below follow those groups.

Three quick checks fix many problems:

1. Check that you are online.
2. Re-import your cookies (see [Cookie Management](cookie-management.md)).
3. Look at the **Activity** page in the sidebar. It shows what MeedyaDL was doing when the problem happened.

---

## Common Errors and Solutions

### Sign-in and Cookie Errors

#### "Authentication failed"

Your Apple Music cookies have run out or are not valid. This is the most common error. It usually means your browser's Apple Music session ended after you last exported cookies.

- **Fix:** Sign in to Apple Music in your browser, export your cookies again, then import the new file in **Settings > Cookies**. See [Cookie Management](cookie-management.md) for step-by-step help.

#### "Cookie file not found"

MeedyaDL is set to use a cookie file that is no longer where it expects. It may have been moved or deleted.

- **Fix:** Import your cookie file again in **Settings > Cookies**. That saves the new location.

---

### When You Are Offline

MeedyaDL checks your internet connection before each download. If you are offline when you add a download, it is still added to the queue. It will not start until you are back online. A message says: **"Download queued — no internet right now. They will start automatically when the connection comes back."**

When you are back online, waiting downloads start by themselves. If you have turned off **Auto-Start Downloads**, press **Start** on the Queue page. Cookies are not checked while you are offline.

Downloads that fail only because of your connection do not create an error report.

---

### Connection Errors

These include timeouts, name-lookup failures, and problems on Apple's side. MeedyaDL tries again by itself up to 3 more times (4 tries in all) before it reports a failure. The tries come straight after each other, so if your connection is down, all of them fail. Wait until you are back online, then retry the item yourself.

#### Timeout or lookup failure

- **Cause:** Your internet is down or unstable.
- **Fix:** Check your connection. Try opening `https://music.apple.com` in your browser. If that works and the error stays, try again in a few minutes.

#### Server errors

- **Cause:** Apple Music is having problems or is being worked on.
- **Fix:** Wait a few minutes and try again. You can check [Apple's System Status page](https://www.apple.com/support/systemstatus/).

#### Behind a firewall, proxy or VPN

MeedyaDL follows your computer's proxy settings. On a work network, make sure Apple Music is allowed. If you use a VPN and downloads fail, try turning it off to see if it is the cause.

#### Downloads stuck at 0%

- Check your internet connection.
- Cancel the download and add it again.
- Open **Settings > Tools** and check that FFmpeg is installed.

---

### Format and Quality Errors

#### "Requested quality not available"

Not every track or video comes in every format.

- **Cause:** The format you chose is not offered for this item.
- **Fix:** Turn on **Enable Fallback Chain** in **Settings > Codec & Resolution**. MeedyaDL then tries the next format on your list. You can also pick a different format before you download. See [Fallback Quality](fallback-quality.md) and [Quality Settings](quality-settings.md).
- **Tip:** ALAC (lossless) is offered for the most tracks.
- **Video:** Video resolution never causes this error. It is a ceiling, so you always get the best the video has at or below your choice.

#### An extra format copy was not made

You turned on **Companion Downloads** but one extra copy is missing.

- **A file with the same name was already there.** Extra copies never replace a file. The Activity page says how many extra copies in that format were not made because a file with the same name was already there. Move or rename the old file, then download again.
- **The format is not offered for that song.** The Activity page says so, and the song is left out of that format.
- **Downloading an album again does not refresh extra copies** that are already in the folder.

See [Quality Settings](quality-settings.md#companion-downloads).

#### Some tracks skipped (partial album)

Only some tracks downloaded, and the Activity page says "Requested format is not available" for the rest.

- **Cause:** The special format you picked (such as Dolby Atmos) is not offered for every track on the album.
- **What MeedyaDL does:** It spots the gaps and downloads the missing tracks in a more common format, without touching files that already worked. The Activity page shows "Gap-fill complete" when this succeeds.
- **If that also fails:** Choose a common format such as **ALAC** or **AAC**. If your lossless downloads need the wrapper, check that it is on (see below).

#### Output file won't play

- **Cause:** Your player does not support the format. This is most common with lossless (ALAC) and high-resolution files.
- **Fix:** Try [VLC](https://www.videolan.org/vlc/), which plays almost everything. For files that work on the most devices, download in **AAC**.

---

### Not Found Errors

#### Content not found

- **Cause:** The item was removed from Apple Music, the link is wrong, or the item is not offered in your region.
- **Fix:** Open the link in your browser at `https://music.apple.com`. If it is gone there, it can't be downloaded.

#### Link from another country

If you paste a link from another country (for example a `/us/` link while your account is in `gb`) and the album is not available there, the download first fails with "Resource Not Found".

- **What MeedyaDL does:** It tries once more with your own region. The Activity page says so, for example: `Storefront 'us' returned no catalog entry — retrying with your account region 'gb'…`. If the album exists in your region, the download works with no more action from you.
- **If both fail:** The Activity page says `Album not available in 'us' or 'gb'`, and the item shows as failed. The album may be region-locked or removed.
- **To turn this off:** Untick **Settings > General > Auto-retry with your region when a URL's storefront fails**.

<details>
<summary>More detail: why the region retry happens only once</summary>

The retry is limited to one go so that two links whose albums are not available in each other's region cannot bounce back and forth forever. Pressing **Retry** on the item gives it a fresh try.

</details>

---

### Too Many Requests

Apple limits how many requests one account can make in a short time. You are most likely to hit it part-way through a big album, playlist or whole discography. The download starts well, and then every remaining track fails.

- **What MeedyaDL does:** It pauses the queue by itself so it does not make things worse. Files already downloaded are kept.
- **Fix:** Wait. The pause usually lasts **1 to 2 hours or more**, not minutes. Then click **Resume** on the Queue page. Only the missing tracks are fetched.
- **To avoid it:** Download in smaller batches, a few albums at a time instead of a whole discography.

---

### Playlist File Problems

If you turn on the option that saves a playlist file (`.m3u8`), the **first** track of a playlist can fail while the rest download fine. This is a known problem in a tool MeedyaDL uses, and the fix has to come from its makers.

- **Fix:** Retry the failed first track on its own, or download the album it belongs to. Turning the playlist-file option off also avoids the problem.

---

### Helper Tool Errors

#### Missing tools

MeedyaDL needs helper programs, such as FFmpeg. If one is missing or damaged, you see a tool error.

- **Fix:** Open **Settings > Advanced > Setup** and press **Re-run Setup Wizard**. It installs what is missing and does not change your other settings. You can also press **Reinstall** next to a tool in **Settings > Tools**.
- **Tools you installed yourself:** If you installed a tool with a package manager (Homebrew, MacPorts, apt, dnf, pipx, Scoop or snap), MeedyaDL uses that copy and does not download another. The wizard marks it with a **Homebrew** or **System** badge. Updates stay with the package manager that installed it. If a tool you installed still shows as missing, check that it is a recent enough version, then reopen the wizard.

#### Python stopped working after a system update

If you told MeedyaDL to use a Python already on your computer, and you later upgrade or move that Python (for example with `brew upgrade python`), MeedyaDL's Python setup can break.

- **Fix:** Reopen the **Python** step of the setup wizard. MeedyaDL notices this and offers a **Rebuild** button. If the old Python is gone, pick another from the list, or let MeedyaDL download its own copy.

#### Downloads fail after trying GAMDL 4

GAMDL 4 has not been tested on real downloads yet, so problems are possible.

- **A message about lyrics:** **Synced Lyrics Only** (Settings > Lyrics) does not work with GAMDL 4. Turn it off.
- **Which version:** use GAMDL 4.0.2 or newer. Versions 4.0.0 and 4.0.1 are marked as having known problems in **Settings > Tools**, and MeedyaDL will not install them.
- **Anything else:** Go back to GAMDL 3.9.1 in **Settings > Tools > GAMDL Version Management**, and please report what happened. See [Trying GAMDL 4](tools.md#trying-gamdl-4).

#### "GAMDL backend not found"

The Python setup or GAMDL itself is damaged or missing.

- **Fix:** Open **Settings > Advanced > Setup** and press **Re-run Setup Wizard**. Your cookies, settings and downloaded files are not touched.

---

### Wrapper Problems

If you use the wrapper, see [Wrapper Authentication](wrapper.md) for how to set it up.

#### "Wrapper unreachable" warning

A yellow message says MeedyaDL cannot reach the wrapper. Check that the wrapper is running, and that the addresses in **Settings > Advanced > Wrapper** are right. The yellow message stays until the wrapper can be reached on a later download, then goes away by itself. You only ever see one wrapper warning at a time.

#### Auto-Retry without Wrapper

If wrapper downloads fail often, turn on **Auto-Retry without Wrapper** in **Settings > Advanced > Wrapper**. A failed wrapper download is then queued again with the wrapper off, using your cookies. With it off, a failed item shows a **Retry without Wrapper** button so you can do this yourself.

#### Every track is skipped when the wrapper is on

If **every** track in a download is skipped (the Activity page lists each one as skipped), the wrapper's sign-in to Apple Music has most likely run out, even though the wrapper looks like it is running. Sign the wrapper in again: with the newer wrapper, press **Sign in to wrapper** in **Settings > Advanced > Wrapper**; with the older one, follow its own instructions. Then retry the download. [Wrapper Authentication](wrapper.md) has the details.

If only **some** tracks are skipped, those tracks are probably not offered in the format you asked for. Adjust your [Fallback Quality](fallback-quality.md) list so another format is tried for them.

<details>
<summary>More detail: other reasons tracks are skipped</summary>

- **The wrapper is switched off.** Some formats, such as Lossless (ALAC), need the wrapper; which ones depends on your GAMDL version (see [Wrapper Authentication](wrapper.md)). Switch on **Use Wrapper** in **Settings > Advanced**, or choose a different format.
- **The track is not offered in that format.** Some regions have different versions of an album, and not every track has a Dolby Atmos mix. A fallback list handles this by itself.
- **The track was replaced.** Apple sometimes replaces a song, for example when it moves to another release. Look the album up in your browser to see if it has a new link.
- **Changing Download Mode does not help.** Downloads sometimes seem to start working after a setting change only because the attempt starts fresh.

</details>

#### Running the wrapper on another device

You can run the wrapper on another computer on your network, such as a Raspberry Pi, and run MeedyaDL on your Mac or PC. MeedyaDL then needs that computer's address in **Settings > Advanced > Wrapper**. If only some of the addresses point at the other computer, downloads start but fail part-way through.

Put the other computer's address in the **Wrapper Account URL** and **Wrapper m3u8 Address** fields. With the newer wrapper there is one **Wrapper URL** field instead. If an address ends in a colon and a number, keep that number unless your wrapper uses a different one. The **Wrapper Decryption Address** can stay blank: MeedyaDL works it out from the wrapper's address. [Run the wrapper on another computer](wrapper.md#run-the-wrapper-on-another-computer) has the full steps.

For example, for a wrapper at `192.168.1.50`, change `127.0.0.1` to `192.168.1.50` in each field and leave the decryption address blank.

Before each download, MeedyaDL checks every address and shows a yellow message naming any it cannot reach.

<details>
<summary>More detail: if the other computer cannot be reached</summary>

- **Check the wrapper accepts connections from other computers,** not only from the computer it runs on. Its own setup instructions say how.
- **Check that computer's firewall** lets the connections through.
- **Check every address field** points at the other computer, not just one of them.

</details>

---

### Retrying Failed or Partial Downloads

#### Retry one item

- **Queue page:** every failed item has a **Retry** button (a circular arrow). Items that used the wrapper also have **Retry without Wrapper**. Right-click a row for the same actions plus **Copy Source Link** and **Open Folder**.
- **History page:** every failed entry has a **Retry** button and a right-click menu. The old entry is kept, and the new download adds its own.

#### Retry all failed items

When any item has failed, the **History** page shows **Retry All Failed (N)** at the top, and the **Queue** page has **Retry all failed (N)** in its **More** menu. A window asks you to confirm. On the History page, repeated links are only queued once.

#### Smart retry: only the missing tracks

When you retry an album that only partly downloaded, MeedyaDL compares the tracks the album should have with the files already in its folder, and fetches only the missing ones. A 50-track box set with 3 failed tracks takes seconds.

- If **all** tracks are already there, the retry says `Nothing to retry — all N track(s) are already on disk`.
- If **some** are missing, only those are fetched. The Activity page says, for example, `Smart retry for {id}: targeting 3 of 50 track(s) — 3 URL(s) queued`.
- If the album folder has no `manifest.meedyadl` file (the first run failed too early to write one), MeedyaDL retries the whole album and keeps the files that already exist.
- The **Library** page does the same for albums you downloaded earlier. Its **Re-download** button fills only the gaps and keeps the songs already there. See [Library Scan](downloading-music.md#library-scan).

---

### App Problems

#### MeedyaDL won't open

##### macOS

Official MeedyaDL downloads are signed and checked by Apple, so macOS should open them with no warning. If you still see "can't be opened", it is almost certainly one of these:

- **An older download.** Disk images made before September 2026 were signed but not fully checked by Apple, so macOS warned about the disk image. Download the current version.
- **You built it yourself.** Builds made outside our release process are not signed, and macOS warns about them.
- **The download was cut short or changed.** Download it again.

If you are on an older download and do not want to update yet:

1. Right-click (or Control-click) the MeedyaDL app and choose **Open**.
2. In the window that appears, click **Open**.
3. If that does not work, go to **System Settings > Privacy & Security**, scroll down and click **Open Anyway** next to the MeedyaDL message.
4. You may need to do this twice the first time.

Please do not run `xattr -cr`. It strips every extra label from the app, not only the one macOS checks. Right-click and **Open** does the same job for one app and nothing more.

##### Windows

Windows SmartScreen may stop the installer or app from running.

1. When the SmartScreen window appears, click **More info**.
2. Click **Run anyway**.
3. If the installer still fails, download it again. Also check that you have Windows 10 or later.

##### Linux

MeedyaDL needs a few system libraries that some Linux systems do not have.

1. Install them. On Debian-based systems:

   ```bash
   sudo apt-get install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev
   ```

   On other systems, use your package manager to install the matching packages (for example `webkit2gtk4.1`, `libappindicator-gtk3`, `librsvg2`).

2. For the AppImage version, make it runnable, then start it:

   ```bash
   chmod +x MeedyaDL.AppImage
   ./MeedyaDL.AppImage
   ```

#### MeedyaDL stops right after it opens

Try these in order:

1. Update to the latest version of MeedyaDL.
2. Delete the `queue.json` file from the app data folder (the folders are listed below). This clears queued and failed downloads from the last session. It does not touch your settings, cookies or finished downloads.
3. If it still won't start, delete `settings.json` from the same folder. MeedyaDL makes a new one with default settings.

| System | App data folder |
| --- | --- |
| macOS | `~/Library/Application Support/com.meedyasuite.meedyadl/` |
| Windows | `%APPDATA%/com.meedyasuite.meedyadl/` |
| Linux | `~/.local/share/com.meedyasuite.meedyadl/` |

#### Settings not saving

Your changes are gone after you restart.

- **Cause:** MeedyaDL cannot write to its app data folder (the table above).
- **Fix:** Make sure your user account can write to that folder. If that does not help, delete `settings.json` there to go back to the defaults.

#### Write errors in the temp folder

Downloads fail with a permission error about a temporary or working folder.

- **Fix:** MeedyaDL uses a temporary folder named `MeedyaDL` inside your system's temp folder. If you still see write errors, open **Settings > Tools**, and make sure the **Temp Directory** is a place you can write to. Clear it to go back to the default.

#### Cloud drives and network folders

If the app seems slow, or you see "Output directory timed out" while saving to a cloud drive:

- **Cause:** The output folder is on a cloud or network drive that is disconnected or slow to answer.
- **What MeedyaDL does:** Before a download starts, it checks that it can write to the output folder (waiting up to 5 seconds), so a stuck drive does not freeze the window.
- **Fix:**
  1. Check that the drive is connected and working.
  2. Download to a folder on your own computer, then move the files to the cloud drive.
  3. If the drive is gone for good, change the folder in **Settings > General > Output**.

#### No output files, though the download looked fine

If a download seems to finish but MeedyaDL says "no output files":

- **Fix:** Update to the latest version. If it still happens, check that the output folder's path has no unusual characters and that you can write to it.

---

### A Feature Says It Is "Temporarily Unavailable"

You may sometimes see a notice at the top of the app that a feature is temporarily unavailable.

**This is on purpose. It is not a problem with your computer or your install.** Now and then we pause a feature for everyone at short notice, for example while we look into a change on the service it uses. The app shows a notice so the feature does not just vanish.

- **You don't need to do anything.** Reinstalling, changing settings or refreshing cookies will not bring it back sooner. It returns by itself.
- **The rest of the app keeps working.** Only the named feature is paused.
- **If the notice has extra text,** it comes from us and usually says why, or when to expect it back.
- **Being offline never causes this.** If MeedyaDL can't check in, it keeps what it last knew.

If a whole download service is paused, MeedyaDL does not start new downloads for it and shows you the reason. A download already running finishes normally, and other services are not affected.

If a notice has been showing for a long time, check [GitHub Releases](https://github.com/MWBMPartners/MeedyaDL/releases) for an app update.

---

### Notifications Don't Appear

Messages always appear inside MeedyaDL. Your computer shows them as system notifications only when the settings in **Settings > General > Notifications** allow it. Check these first:

- **System Notifications** is on, and **Notification Style** is not **In-app only**.
- **Show System Notifications**: by default a system notification is shown only while MeedyaDL is not in front (behind another app, minimised or hidden in the tray) or while your pointer is on another screen. While you are looking at MeedyaDL, the message appears inside the app only. **Only when the pointer is on another screen** never shows one if you have a single screen.
- **Which Messages**: by default only important messages (downloads finishing or failing, the queue finishing, a copied link, a shut-down warning, an app update) become system notifications.
- The switch for that kind of event (for example **A Download Finished**) is on.
- You pressed **Save Changes**. The app acts on saved settings.

Then press **Send Test Notification**. If the test does not appear, the setting to change is on your computer:

- **macOS:** open **System Settings > Notifications > MeedyaDL** and allow notifications. Check that a Focus mode (such as Do Not Disturb) is not on. A development build of MeedyaDL shows its notifications as **Terminal**, so look under Terminal instead.
- **Windows:** open the Windows Settings app, go to **System**, then **Notifications**. Make sure notifications are on, and on for MeedyaDL, and that **Do not disturb** is off.
- **Linux:** notifications are shown by your desktop's notification service. Most desktops (GNOME, KDE Plasma, Cinnamon, XFCE) run one for you. A minimal window manager may not, so install and start one (for example `dunst` or `mako`). Also check your desktop's own notification settings and any do-not-disturb mode.

<details>
<summary>More detail: checking what MeedyaDL thinks your system is</summary>

**Settings > Advanced > Diagnostics** shows which system MeedyaDL thinks it is on and whether it is a development build. On Windows it also shows the name your notifications are listed under. A copy run straight from a build folder has its notifications listed under PowerShell instead.

</details>

---

## The Activity Page and Log Files

### Reading the Activity page

The **Activity** page (sidebar, or Cmd/Ctrl + K) lists what MeedyaDL does, as it happens. Lines marked `[System]` are about the app itself. Others belong to a download.

- **Auto-scroll** is on by default, so the page follows new lines. If you scroll up to read earlier lines, the switch turns itself off so your place is kept. Turn it back on to jump to the bottom.
- The page holds up to **10,000 lines** per session. Older lines are dropped from the view but are kept in the log files (see below).
- When a download starts, the page says how it signs in: **"Authentication: Wrapper (…)"** or **"Authentication: Cookie-based (no wrapper)"**. This is normal. If a download fails with a sign-in error and the line says cookies, refresh your cookies. If it says wrapper, check that the wrapper is running.
- Turn on **Verbose Activity Log** in **Settings > Advanced** for extra detail. This is the best first step when a problem is about tags or formats. On a full (Stable) release it turns itself off every time you restart MeedyaDL, so you need to turn it on again each time. On Alpha, Beta and Release Candidate builds (the version has `-alpha`, `-beta` or `-rc` in it), it stays on.

### Saving the log

The **More** menu at the top of the Activity page has three actions:

| Menu item | What it does | When to use it |
| --- | --- | --- |
| **Export the lines shown** | Saves the lines you can see now. Any search or filter you set is applied. | Sharing a small part of the log. |
| **Export the full log (last 3 days)** | Saves the complete record of the last three days from your disk. | Bug reports. It includes lines that are no longer on screen, and every detailed line even if the Verbose filter is off. |
| **Open the logs folder** | Opens the folder with the log files. | Attaching the files to a bug report. |

To save what you see: open **More**, choose **Export the lines shown**, then pick where to save. The file name includes the date and time, for example `MeedyaDL-activity-log_2026-04-22_14h05m.log`. It is plain text, one entry per line. To save the whole list, first clear the search box and tick all the category filters (System, Download, Verbose).

### Log File Locations

MeedyaDL keeps its log files in a `logs` folder inside the app data folder:

| System | Log folder |
| --- | --- |
| macOS | `~/Library/Application Support/com.meedyasuite.meedyadl/logs/` |
| Windows | `%APPDATA%/com.meedyasuite.meedyadl/logs/` |
| Linux | `~/.local/share/com.meedyasuite.meedyadl/logs/` |

The easiest way to get there is **Open the logs folder** in the Activity page's **More** menu.

You can keep the activity log files somewhere else, for example on an external drive, in **Settings > Advanced > Diagnostics > On-disk activity log location**. Click **Browse…** to pick a folder, or **Reset** to go back to the default. The change applies the next time you start MeedyaDL. If the folder can't be written to, MeedyaDL uses the default instead.

<details>
<summary>More detail: what each log file holds and how long it is kept</summary>

| File name | What it holds | Kept for |
| --- | --- | --- |
| `meedyadl.YYYY-MM-DD.log` | The app's own detailed log. | 7 days |
| `activity-YYYY-MM-DD.log` | Everything shown on the Activity page, including lines trimmed from the on-screen list and every detailed line. | 7 days |
| `session-YYYY-MM-DD.log` | An older record of lines that were trimmed from the on-screen list. Mostly replaced by the `activity-` files. | 30 days |

A busy day usually writes under 5 MB. The `meedyadl.*` and `session-*` files always stay in the default folder, even if you move the activity log.

Entries in the app's own log start with a time, a level and a name. Levels, from most to least serious:

| Level | Meaning |
| --- | --- |
| **ERROR** | Something failed. Look here first. |
| **WARN** | A possible problem. The step may still have worked. |
| **INFO** | Normal progress, such as a download starting or finishing. |
| **DEBUG** | Fine detail for troubleshooting. Only shown when detailed logging is on. |

When you look for a problem, search for **ERROR** first. The time on the line tells you which download it belongs to, and the lines just before it show what the app was doing.

</details>

<details>
<summary>More detail: turning on detailed logging when you start the app</summary>

MeedyaDL writes only normal (INFO) detail to its log files. To get more, set the `RUST_LOG` environment variable to `debug` before you start the app:

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

This makes the log files grow quickly, so only use it while troubleshooting. Start the app normally afterwards to go back to normal detail. It is separate from the **Verbose Activity Log** switch, which only affects the Activity page.

</details>

---

## Crash Reports

If MeedyaDL crashes or hits an unexpected error, it saves a short report on your computer. It contains the error message, technical details, the app version and your system type. Reports are named `crash-YYYYMMDD-HHMMSS.json` and are deleted after 30 days.

| System | Crash report folder |
| --- | --- |
| macOS | `~/Library/Application Support/com.meedyasuite.meedyadl/crashes/` |
| Windows | `%APPDATA%/com.meedyasuite.meedyadl/crashes/` |
| Linux | `~/.local/share/com.meedyasuite.meedyadl/crashes/` |

### Anonymous Crash Reporting (Optional)

You can choose to send crash reports to help improve MeedyaDL. They include the error message, technical details, app version and your system type. They never include personal details, download history or account information. This is off unless you say yes.

MeedyaDL asks you once, in a **Help Improve MeedyaDL** window: **Yes, send crash reports** or **No thanks**. Closing the window counts as No thanks. Your answer is remembered, so you are not asked again. If MeedyaDL could not save your answer, it tells you and asks again next time. You can change your mind any time with the **Send Anonymous Crash Reports** switch in **Settings > Advanced > Error Reporting**. The change applies after you restart MeedyaDL.

### Reporting a Crash via GitHub Issues

You can send a crash to the developers from inside MeedyaDL. This opens a ready-filled GitHub issue in your browser. You need a [GitHub account](https://github.com/signup).

1. Go to **Settings > Advanced > Error Reporting**.
2. Recent crash reports are listed under the crash reporting switch, each with its date, time and a short summary.
3. Click **Report** next to the one you want to send.
4. A preview shows exactly what will be included: the error message, technical details, app version, operating system and time. Check you are happy to share it.
5. Click **Open GitHub Issue**. Your browser opens a ready-filled issue.
6. Add anything else that helps, such as what you were doing, then click **Submit new issue**.

Good to know:

- No personal details, download history, cookies or account information are included.
- Very long technical details are shortened automatically so they fit in the link.
- Use **Delete** in the same list to remove reports you no longer need.

---

## Reporting a Bug

If your problem is not covered here, or the fixes did not work, please report it:

1. **Note the app version.** It is in **Help > About**.
2. **Copy the relevant log lines.** Use **Export the full log (last 3 days)** (see above) and copy the ERROR lines with the lines around them. If you can, turn on **Verbose Activity Log**, repeat the problem, and include the extra lines.
3. **Write down the steps.** Say what you did, the link you used, and the quality settings you had chosen.
4. **Open an issue on the GitHub page.** Include the app version, your operating system and version, the log lines and your steps. The more detail you give, the faster it can be fixed.

---

## Related Topics

- [Cookie Management](cookie-management.md) — Fixing sign-in and cookie problems
- [Wrapper Authentication](wrapper.md) — Setting up and fixing the wrapper
- [Quality Settings](quality-settings.md) — Understanding quality and format options
- [Fallback Quality](fallback-quality.md) — Setting up automatic fallback
- [FAQ](faq.md) — Frequently asked questions
- [Getting Started](getting-started.md) — First setup

---

[Back to Help Index](index.md)
