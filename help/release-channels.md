<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Release Channels

MeedyaDL publishes builds on four release channels, ordered from **least stable** to **most stable**. You pick one in **Settings > General > Updates** and the in-app updater stays on that channel.

## The four channels

| Channel | Cadence | Version suffix | Who it's for |
| ------- | ------- | -------------- | ------------ |
| **Alpha** | Ad-hoc | `-alpha.N` | Feature-complete previews with known rough edges. Hidden from the channel picker unless developer access is unlocked. |
| **Beta** | Ad-hoc | `-beta.N` | Polishing-stage features, closer to Stable than Alpha. |
| **RC** (Release Candidate) | Ad-hoc | `-rc.N` | Final validation pass before a Stable release. |
| **Stable** | Release-please merges | _(no suffix)_ | Most users. Production-ready. |

Picking a channel opts you into everything at least that stable: choosing Beta also lets Stable and RC releases reach you, but nothing less stable than Beta.

## Switching channels

1. Open **Settings > General > Updates**.
2. Pick a channel from the **Update Channel** dropdown.
3. Save. The app will start offering updates from that channel on the next update check.

Moving to a less-stable channel (e.g., Stable → Beta) is always an explicit choice. Moving back up (e.g., Beta → Stable) is equally explicit — the app will not auto-upgrade your channel selection.

## The auto-update guard

MeedyaDL will never **auto-downgrade** your stability tier. Concretely:

- The update check only ever shows you a release from your selected channel (or a more stable one).
- If somebody hands you a URL or deep link pointing at a less-stable build (for example, an Alpha link while you're on Stable), the installer refuses to apply it and surfaces a clear error instead. Change channel first if you actually want that build.

## Which channel should I use?

- **Stable** — pick this unless you have a reason not to. It receives fewer updates but has been tested in the other channels first.
- **RC** — pick this if you want to help catch problems in the last build before it becomes Stable.
- **Beta** — pick this if you want to help shake out features a bit earlier, before they reach RC.
- **Alpha** — only if you're comfortable filing bug reports and rolling back to a working build. Expect regressions. Requires developer access to select.

## Reporting problems on pre-release builds

When reporting an issue for any pre-release channel, please include:

- The **exact version** shown in _Help > About_ (includes the channel suffix, e.g., `1.13.0-alpha.56`).
- Your selected **Update Channel**.
- Reproduction steps and relevant log output (Settings > Advanced > Open Log Folder).

See [Troubleshooting](troubleshooting.md) for log-collection tips.
