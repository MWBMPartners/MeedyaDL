<!--
  MeedyaDL Help Documentation
  Copyright (c) 2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :twisted_rightwards_arrows: Release Channels

MeedyaDL publishes builds on six release channels, ordered from **least stable** to **most stable**. You pick one in **Settings > General > Updates** and the in-app updater stays on that channel.

## The six channels

| Channel | Cadence | Version suffix | Who it's for |
| ------- | ------- | -------------- | ------------ |
| **Nightly** | Daily at 00:00 UTC | `-nightly.YYYYMMDD` | Developers validating today's work-in-progress. Can be broken. |
| **Weekly** | Sunday at 00:00 UTC | `-weekly.YYYYWW` | Testers who want to try a week's worth of nightlies rolled up. |
| **Monthly** | 1st of the month at 00:00 UTC | `-monthly.YYYYMM` | Early adopters wanting a monthly preview. |
| **Alpha** | Ad-hoc | `-alpha.N` | Feature-complete previews with known rough edges. |
| **Beta** | Ad-hoc | `-beta.N` | Release candidates — functional and close to Stable. |
| **Stable** | Release-please merges | _(no suffix)_ | Most users. Production-ready. |

Each channel is an integration of the channel directly below it plus any ready `feat/*` branches, so moving up the ladder picks up strictly more testing and integration.

## Switching channels

1. Open **Settings > General > Updates**.
2. Pick a channel from the **Update Channel** dropdown.
3. Save. The app will start offering updates from that channel on the next update check.

Moving to a less-stable channel (e.g., Stable → Nightly) is always an explicit choice. Moving back up (e.g., Nightly → Stable) is equally explicit — the app will not auto-upgrade your channel selection.

## The auto-update guard

MeedyaDL will never **auto-downgrade** your stability tier. Concretely:

- The update check only ever shows you a release from your selected channel.
- If somebody hands you a URL or deep link pointing at a less-stable build (for example, a Nightly link while you're on Stable), the installer refuses to apply it and surfaces a clear error instead. Change channel first if you actually want that build.

## Which channel should I use?

- **Stable** — pick this unless you have a reason not to. It receives fewer updates but has been tested in the lower channels first.
- **Beta** — pick this if you want to help shake out release candidates a few days before they reach Stable.
- **Alpha / Monthly / Weekly / Nightly** — pick these if you are comfortable filing bug reports and rolling back to a working build. Expect regressions.

## Reporting problems on pre-release builds

When reporting an issue for any pre-release channel, please include:

- The **exact version** shown in *Settings > About* (includes the channel suffix, e.g., `0.34.6-nightly.20260420`).
- Your selected **Update Channel**.
- Reproduction steps and relevant log output (Settings > Advanced > Open Log Folder).

See [Troubleshooting](troubleshooting.md) for log-collection tips.
