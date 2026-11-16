<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Downloading Music

This guide shows you how to download songs, albums, playlists and artist collections from Apple Music with MeedyaDL, and how to manage your queue.

---

## Overview

To download music, paste a link from Apple Music into the box on the **Download** page and press **Add to Queue**. MeedyaDL does the rest. It saves the music in the quality you chose, adds the title, artist, cover and lyrics, and puts the files in your output folder. If a quality is not available for a track, it tries the next best one by itself.

You can paste several links at once, one per line. If you keep your links in a plain text file, the **Link List** button beside the box adds every link in the file (one link per line). **Import** brings in a MeedyaDL download record (a `.meedyadl` file), and **Scan** looks through a folder for those records. See [Download Manifests](#download-manifests-meedyadl-files).

Before you start, make sure you have finished setup (see [Getting Started](getting-started.md)) and that you are signed in (see [Cookie Management](cookie-management.md)).

---

## Supported URL Types

MeedyaDL works out what a link is from its address. Links from `music.apple.com` work, and so do links from `classical.apple.com` and `itunes.apple.com`.

### Songs

A link with `/song/` in it downloads one track. To get it, open the song in Apple Music, choose the share or copy-link option, and paste the link into MeedyaDL.

**Example:** `https://music.apple.com/us/song/track-name/1234567890`

### Albums

A link with `/album/` in it downloads every track on the album. The files go into an album folder under the artist's folder, and the cover is added to every track. Albums are the easiest way to get a whole release.

**Example:** `https://music.apple.com/us/album/album-name/1234567890`

### Playlists

A link with `/playlist/` in it downloads every track in the playlist. Each track is saved under its own artist and album. If one track cannot be downloaded (for example, it is not available in your country), the rest carry on.

**Example:** `https://music.apple.com/us/playlist/playlist-name/pl.1234567890`

### Artists

A link with `/artist/` in it downloads from an artist's catalogue. This can be a lot of music. An artist link becomes one queue item, or one per kind of release you ticked (see below).

You can narrow it down with **Artist Auto-Select** in **Settings > Codec & Resolution**. It lets you choose which kinds of release to take (Main Albums, Singles & EPs, Music Videos and so on). If you pick more than one, MeedyaDL adds one queue item for each. For example, picking "Main Albums" and "Singles & EPs" gives two queue items. A song that appears in more than one of them is only downloaded once. The **Duplicate Detection** settings in the same place control this.

**Example:** `https://music.apple.com/us/artist/artist-name/1234567890`

### Library URLs

Links to your own Apple Music library also work. They look like `music.apple.com/library/...`. Paste them like any other link. This is handy for music you added to your library yourself.

**Example:** `https://music.apple.com/library/albums/l.1234567890`

---

## Using the Download Page

### Adding links

Paste a link into the box. MeedyaDL works out whether it is a song, album, playlist or artist, so you do not need to say. A link to a service MeedyaDL cannot download from yet is not added, and the form tells you so. See [Supported Services](supported-services.md).

To add several at once, paste one link per line. **Shift + Enter** starts a new line. **Enter** or **Add to Queue** adds them all. You can keep adding while earlier downloads are running.

MeedyaDL starts only about ten new downloads a minute, to keep things steady. If you paste more than that, the first ones are added and a message says how many were added and how many were not. The rest stay in the box, so press **Add to Queue** again after the wait the message gives. Any link that could not be added for another reason also stays in the box. Nothing disappears.

### Picking the quality

Before you add a link, you can choose a different audio quality for it from the quality picker on the form. If you do not, your default from [Quality Settings](quality-settings.md) is used.

| Quality | What it is |
| --- | --- |
| **AAC** | The usual streaming quality. A good balance of sound and file size. |
| **AAC-HE** | A smaller, lower-quality version. Good if space is tight. |
| **AAC Binaural** | Surround sound made to sound spatial on headphones. |
| **AAC Downmix** | A stereo version of surround sound. |
| **AAC Legacy** | A version that works on older devices and software. |
| **ALAC** | Lossless. Identical to the original, with bigger files. Good for archiving. |
| **Atmos** | Dolby Atmos spatial sound, for players that support it. |
| **AC3** | Dolby Digital 5.1 surround sound. |

For more on what these mean, see [Audio Codecs](audio-codecs.md).

### Managing the queue

Added links go into the queue. By default the queue starts at once (**Auto-Start Downloads** in **Settings > General**). If you would rather add a lot first and start later, turn that off. Items then wait as "Queued" until you press **Start** at the top of the **Queue** page.

MeedyaDL does one item at a time on purpose. Everything for one item (the music, any extra formats, tags and lyrics) finishes before the next one begins. That keeps retries and extra formats reliable. There is no setting to run several at once.

Each item in the queue shows:

- A **progress bar** while it is downloading.
- Its **status**: fetching details, downloading, tagging, finished or failed.
- A **Fallback used** note if MeedyaDL had to start the whole item again in another quality.

What you can do with the queue:

- **Cancel** stops one item straight away. Any extra format copies it was making are stopped too.
- **Retry** tries a failed item again. If part of an album is already saved, MeedyaDL downloads only the tracks that are missing. If every track is already there, it tells you instead of downloading again.
- **Retry without Wrapper** (only on items that used the wrapper) tries again using your cookies instead. See [Wrapper Authentication](wrapper.md).
- **Retry all failed** (in the **More** menu) retries every failed item. It asks you to confirm first and shows how many.
- **Right-click an item** for a menu: Copy Source Link, Open Folder (once files exist), Retry, and Retry without Wrapper where it applies.
- **Clear completed** (in the **More** menu) removes finished and cancelled items. Failed items stay, so you can read what went wrong. Use **Clear all** to remove those too.
- **Clear all** (in the **More** menu) asks you to confirm, then removes every item that is not running. Anything downloading right now keeps going.
- **Pause** and **Resume** (at the top of the page). Pause lets the current item finish and starts nothing new. Resume carries on.
- **Abort** (at the top of the page) stops the current download straight away and cancels everything waiting. Finished downloads are kept. It asks you to confirm. Tick **Don't ask again** in that window to skip the question, and turn it back on in **Settings > General > Preferences > Confirm before aborting the queue**. The status bar at the bottom has an Abort button too, and the shortcut is **Cmd/Ctrl + Shift + .**. If the abort cannot be carried out, MeedyaDL tells you.
- **Export the queue** and **Import a queue file** (in the **More** menu). See [Queue Export and Import](#queue-export-and-import).

The **History** page has Retry and Retry All Failed buttons too, for items that have already left the queue. Retrying from History adds a fresh queue item and keeps the old History entry.

**Clear History** on the History page asks you to confirm and cannot be undone. MeedyaDL uses your history to spot repeats and to tell you when you have downloaded something before, so clearing it removes that memory too.

<details>
<summary>More detail: the queue is saved if the app closes</summary>

MeedyaDL saves the queue every time it changes. If the app closes or crashes, the queue comes back when you open it again.

- Items that were waiting or running are picked up and carried on automatically after a short delay. This happens whatever your auto-start setting is.
- Failed items come back as failed, with the original message, so you can read it and retry when you are ready. They are never retried by themselves.
- Finished and cancelled items are not kept.
- The queue is stored in a file called `queue.json` in the app's data folder.

</details>

### After the queue finishes

MeedyaDL can do something by itself when the whole queue is done: open the output folder, play a sound, close MeedyaDL, or restart, sleep (hibernate) or shut down the computer.

Whatever you choose, MeedyaDL also tells you the queue has finished, and how many downloads worked and how many failed.

- **Every time:** choose it in **Settings > General > Preferences > After Queue Completes**, then save.
- **Just this once:** right-click an empty part of the Download page and pick one of the **After Queue:** options. It is saved straight away. It is used the next time the queue finishes and then cleared, and until then it takes the place of your usual setting.

The status bar at the bottom shows what will happen, for example "After queue: Shut down (once)". If MeedyaDL cannot save a one-off choice, it tells you, and says what will really happen instead, so you are not caught out when you leave your computer running.

<details>
<summary>More detail: what each option does on your system</summary>

- **Play a sound** plays your computer's own alert sound, whether or not MeedyaDL is in front. On Linux it uses your desktop's sound player if you have one. If you do not, no sound plays.
- **Restart or shut down:** on Windows, MeedyaDL gives you 30 seconds' warning first. On macOS and Linux it starts straight away, and the message says so as it begins.
- **Sleep** (hibernate on Windows) starts straight away on every system.

</details>

### Queue Export and Import

You can move a queue to another computer, or keep a copy of it before you reinstall.

**To export:**

1. Open the **More** menu at the top of the Queue page and choose **Export the queue**. It is greyed out when there is nothing active or waiting.
2. Choose where to save it. The suggested name is `queue.meedyadl`.

The file holds the links and any quality choices you made for individual downloads. It does not hold your general settings.

**To import:**

1. Open the **More** menu and choose **Import a queue file**.
2. Pick a `.meedyadl` file.
3. The items are added to your queue. They start at once if auto-start is on, or wait for **Start** if it is off.

Imported items use your own settings, with any per-download choices from the file applied on top.

---

## Progress and Errors

While a download runs you can see the current track, a progress bar, and what it is doing (fetching details, downloading, tagging). When it finishes, the item is marked as done. If something goes wrong, it is marked as failed with a message that says why.

| Kind of problem | What it means | What to do |
| --- | --- | --- |
| **Sign-in (auth)** | Your cookies are out of date or missing. | Sign in again. See [Cookie Management](cookie-management.md). |
| **Network** | The connection dropped or timed out. | MeedyaDL tries four times in total, one straight after another. If all fail, check your internet and press Retry. |
| **Quality (codec)** | The quality you chose is not available for this track. | The fallback runs by itself. If every quality fails, the track may not be downloadable. |
| **Not found** | The music has been removed from Apple Music. | Nothing can fix this. |
| **Too many requests (rate limit)** | Apple Music is asking MeedyaDL to slow down. | MeedyaDL pauses the whole queue so it does not make things worse. The wait is usually one to two hours or more. Press **Resume** on the Queue page afterwards. Finished files are kept. |

For more help, see [Troubleshooting](troubleshooting.md).

---

## Output Files

### File names and folders

Files are saved in the output folder you set in Settings. By default they are sorted into **Artist / Album / Track** folders. You can change the layout in **Settings > Templates**.

```text
Output Directory/
  Artist Name/
    Album Name/
      01 Track Title.m4a
      02 Track Title.m4a
      ...
```

All songs are saved as `.m4a` files, whatever the format.

#### Track and disc number padding

Two settings at the bottom of **Settings > Templates** decide how track and disc numbers are padded with zeros:

- **Track Number Padding** (default **Auto**). Auto matches the album: `01` to `99` for most albums, `001` and up for big box sets. You can also pick a fixed width. Padding keeps tracks in the right order in your file list. Without it, track 10 would sort before track 2.
- **Disc Number Padding** (default **Auto**). Auto uses one digit for most albums and two for box sets with ten or more discs.

Changing padding affects new downloads only. Files you already have are not renamed.

### Tags and lyrics

Each file is filled in with its title, artist, album artist, album, track and disc numbers, year, genre, cover picture, and copyright and label details.

Lyrics are set up in **Settings > Lyrics**. By default MeedyaDL saves word-by-word lyrics as an **Enhanced LRC** file and also puts them inside the music file. The formats are **LRC**, **SRT** and **TTML**. For the full story, see [Lyrics and Metadata](lyrics-and-metadata.md).

---

## Fallback Quality

If the quality you chose is not available for a track, MeedyaDL tries the next one in a list. The default order is:

ALAC, Atmos, AC3, AAC Binaural, AAC, AAC Legacy

If MeedyaDL has to start the whole item again in another quality, the queue item shows **Fallback used** with that quality. If only some tracks fall back, the queue does not show it. Each file's name shows its real format. To change the order, see [Fallback Quality](fallback-quality.md).

### Companion Downloads

MeedyaDL can also download extra versions of the same music in other qualities. Choose how in **Settings > Codec & Resolution > Companion Downloads**.

By default (**Atmos → Lossless (ALAC)**), a Dolby Atmos download also gets a lossless (ALAC) copy. Other modes add lossy AAC copies as well. **Custom…** lets you tick exactly the qualities you want.

Extra versions get a short tag in the file name, for example `[Lossless]` or `[Dolby Atmos]`, so they never overwrite each other. Extra versions are made before the next item in the queue starts, and each one gets its own lyric files. You can follow them on the **Activity** page. See [Quality Settings](quality-settings.md#companion-downloads) for what each mode does.

---

## Download Manifests (.meedyadl Files)

After an album downloads, MeedyaDL saves a small record of it in the album folder, named `manifest.meedyadl`. It remembers where the album came from, so you can download it again later without hunting for the link.

```text
Output Directory/
  Artist Name/
    Album Name/
      01 Track Title.m4a
      02 Track Title.m4a
      manifest.meedyadl
```

<details>
<summary>More detail: what a manifest holds</summary>

The manifest is a text file. It holds the album's link, each track's details (title, artist, quality) and any per-download choices that were active at the time.

</details>

### Downloading again from a manifest

There are three ways:

1. **Import button** on the Download page. Pick a `.meedyadl` file. Its links go into the link box, one per line, so you can look them over. Press **Add to Queue** to download.
2. **Drag and drop.** Drop a `.meedyadl` file onto the MeedyaDL window. MeedyaDL checks the file first. If it is not one it understands, or has no links, you are told and nothing changes. Otherwise the links go into the link box, as with the Import button. Up to 500 links are taken from one file. If there are more, a message says only the first 500 were used.
3. **Import a queue file** in the **More** menu on the Queue page. It also accepts these files. Items go straight into the queue.

Importing on the Download page only fills in the links. The downloads use your current settings plus any choices you make on screen. Importing on the Queue page also restores the per-download choices that were saved in the file.

You can also look through a whole folder for manifests with the **Scan** button on the Download page, or with the **Library** page in the sidebar.

---

## Smart Re-Download Detection

When you add an album you have downloaded before, MeedyaDL shows a short message with the date of the earlier download. You can then decide whether to go ahead. It is on by default, and you can turn it off in **Settings > General > Preferences**.

### What it can spot

MeedyaDL compares what Apple Music says about the album now with what it recorded when you downloaded it. It can spot:

- **Better quality.** For example, the album has since gained Dolby Atmos or Apple Lossless.
- **New tracks.** Bonus tracks or a bigger edition.
- **Corrections.** Fixed artist credits, titles or genres.
- **Apple Digital Master.** The album has since earned this label.

### What it cannot spot

It cannot tell when the audio was quietly replaced without any change to the album's details. If you suspect a remaster, download the album again yourself.

---

## Clipboard Monitoring

MeedyaDL can watch for Apple Music links that you copy while the app is open. When you copy one, a message appears in the app offering to download it. Press **Download** on the message to add it to the queue, using your current settings. If it is already in the queue, nothing is added and the message says so.

The message is called **Copied Apple Music Album link found** (or Song, Playlist and so on). It stays inside the app even if you chose **System only** in the notification settings, because it has the **Download** button. While MeedyaDL is in front it goes away after 10 seconds. If MeedyaDL is not in front, or a system notification was sent, it stays until you close it.

Your computer may also show its own notification, following **Settings > General > Notifications**. By default this only happens while MeedyaDL is not in front. That notification has no button, so open MeedyaDL and use the message there. To stop the notification for copied links, turn off **A Copied Link Was Found**.

### Privacy

MeedyaDL only checks whether what you copied looks like an Apple Music link. It does not store or log anything you copy. Anything else you copy is ignored at once. It checks every two seconds.

### Turning it off

It is on by default. To turn it off, go to **Settings > General > Preferences** and switch off **Clipboard Monitoring**.

The same link is only offered once each time you run the app, even if it stays on the clipboard.

---

## Tips

- **Check your sign-in before a big batch.** If your cookies run out partway through a long playlist, the rest of it fails. See [Cookie Management](cookie-management.md).
- **ALAC for archiving, AAC for everyday listening.** ALAC files are bigger (often 30 to 50 MB a track). AAC files are around 7 to 10 MB a track and sound fine to most people.
- **Use album links where you can.** One album link is better than many song links.
- **Watch for the Fallback used note.** If it appears often, the quality you chose is rarely available. Think about changing your default in [Quality Settings](quality-settings.md).
- **Your queue survives restarts.** You do not need to save or re-enter links.
- **Moving to another computer?** Export the queue and import it there. It will use that computer's settings.
- **Want to add lots first?** Turn off **Auto-Start Downloads** in **Settings > General**, add everything, then press **Start**.

---

## Related Topics

- [Quality Settings](quality-settings.md) — Choose your audio quality
- [Fallback Quality](fallback-quality.md) — How MeedyaDL picks another quality when needed
- [Lyrics and Metadata](lyrics-and-metadata.md) — Lyrics, cover art and tags
- [Downloading Videos](downloading-videos.md) — Download music videos
- [Troubleshooting](troubleshooting.md) — Fix common problems

---

[Back to Help Index](index.md)
