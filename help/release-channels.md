<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Release Channels

MeedyaDL comes in four release channels, from **least finished** to **most finished**. You pick one in **Settings > General > Preferences**, in the update settings near the bottom of that section. The app then offers you updates from that channel.

**If you are not sure, stay on Stable.**

## The four channels

| Channel | Who it is for |
| --- | --- |
| **Alpha** | Early previews that still have rough edges. Not shown in the picker by default. |
| **Beta** | Features that are nearly ready. Closer to Stable than Alpha. |
| **RC** (Release Candidate) | The last check before a release becomes Stable. |
| **Stable** | Most people. The finished, tested version. |

Choosing a channel also includes everything more finished than it. For example, Beta also offers you RC and Stable releases, but never Alpha ones.

## Switching channels

1. Open **Settings > General** and scroll to the update settings at the bottom of **Preferences**.
2. Choose a channel from the **Update Channel** list.
3. Save. The next update check uses the new channel.

Moving to a less finished channel is always your choice, and the app asks you to confirm. Moving back is your choice too. MeedyaDL never changes your channel for you.

## You are never moved to a less finished version

MeedyaDL will not switch you to a less finished build by itself.

- An update check only shows releases from your channel, or a more finished one.
- If someone sends you a link to a less finished build (for example an Alpha link while you are on Stable), the installer refuses it and says why. If you really want that build, change your channel first.

## Which channel should I use?

- **Stable.** The best choice for nearly everyone. Updates come less often, and they have been tested in the other channels first.
- **RC.** Choose it to help catch problems in the very last build before Stable.
- **Beta.** Choose it to try new features a little earlier, before they reach RC.
- **Alpha.** Only if you are happy to report problems and go back to a working version. Expect things to break.

<details>
<summary>More detail: how builds are named</summary>

The version number shows the channel. Stable has no ending, for example `1.13.0`. The other channels add an ending:

- Alpha: `-alpha.N`, for example `1.13.0-alpha.56`
- Beta: `-beta.N`
- RC: `-rc.N`

New builds in Alpha, Beta and RC come out whenever new work is ready, not on a fixed schedule.

</details>

## Reporting problems on pre-release builds

If you find a problem on Alpha, Beta or RC, please include:

- The **exact version**, as shown on the **About** page in Help (it includes the channel ending, for example `1.13.0-alpha.56`).
- Which **Update Channel** you chose.
- The steps that cause the problem, and the relevant lines from the logs. The **Open folder** button in **Settings > Advanced > Diagnostics** shows where the logs are.

See [Troubleshooting](troubleshooting.md) for more on collecting logs.

[Back to Help Index](index.md)
