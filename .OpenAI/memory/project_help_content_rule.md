---
name: In-app help content rule (end-user guide only)
description: The maintainer's 16 Nov 2026 rule — help/*.md, fold-outs included, is an end-user guide and must never explain how MeedyaDL obtains content from services.
type: project
---
Set by the maintainer on 16 Nov 2026, during the in-app help rewrite.

**The rule.** The in-app help (`help/*.md`) — including every fold-out "More detail" section — is reserved for end-user guides and configuration guidance. It must never expose how MeedyaDL provides its features internally, above all how it obtains or downloads content from a service provider's platform (Apple Music, Spotify, YouTube, BBC iPlayer, ...). That rules out: the download engines' internals, decryption or unlocking, DRM, device files and their role, wrapper internals (what each address or socket carries, endpoints, setup scripts), tokens, API endpoints or hosts, request identities (user agents), streams and playlists, workarounds for protections, and similar.

**What stays.** Configuration guidance described by its effect for the person: what a setting does, when to change it, real labels and menu paths, how to check something works, what to do when it goes wrong. Ordinary "is the wrapper program running? restart it" steps are operating guidance and are allowed.

**Shape of a page (decided the same day).** Lead with plain wording for everyday users; put technical-but-allowed detail in `<details><summary>More detail</summary>` blocks at the end of the relevant part — a blank line after `<summary>` and before `</details>`, never nested. The maintainer chose fold-outs over an "Advanced" switch or separate advanced pages. A link a test clicks must stay in visible text (a link inside a closed fold-out cannot be reached).

**Scope.** README.md and other repository documents are developer-facing and outside this rule, except where they link into the help.

**History.** All 20 pages were rewritten on 16 Nov 2026 (commits `ea3308ad`, `9bb30b4c`, `f6abe599`, review fixes `0e1fb8fd`); removed passages were listed per page in the session for the maintainer to check.
