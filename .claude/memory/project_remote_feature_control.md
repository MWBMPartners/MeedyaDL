---
name: Remote feature control — intAppsAPI handover, threat model, suite contract
description: Why MeedyaDL's kill switch is moving from a static file to MWBM-intAppsAPI, what was verified about the current state, the honest security limits, and the contract-first rollout across the app suite
type: project
---

Written 2026-07-27. Full analysis lives at `.claude/analysis/remote-feature-flags-analysis.md` (~50KB, every claim tied to a named file at a named revision). Issues: MeedyaDL#1069, MWBM-intAppsAPI#107, MeedyaConverter#465, MeedyaManager#195.

## The goal

Developers need to disable a feature remotely — for upstream breakage or **legal reasons** — without shipping an app update, across the whole MWBM Partners product range. MWBM-intAppsAPI exists to serve that.

## Framing — get this right

The `raw.githubusercontent.com/.../service-status.json` transport in MeedyaDL was a **deliberate interim solution**, adopted knowingly while the API was built out. It is **not** a defect, an oversight, or accidental technical debt. Write it up as a planned handover, never as remediation. Its payload shape is **part of** the basis for the API's model — not a spec to reproduce verbatim.

## What was actually verified (not inferred)

The mechanism **has never run end-to-end in any shipped build**:

- `check_service_status` is registered in `lib.rs` but nothing in the frontend calls it.
- `serviceStatusStore.ts` and `ServiceStatusBanner.tsx` exist; the banner is never rendered anywhere.
- The enforcement helpers in `service_dispatch.rs` have zero call sites outside their own module.
- The hard-coded URL points at `main`, where `service-status.json` **does not exist** — it exists only on `alpha`. So the fetch 404s.

**Consequence, and it is a good one:** there is no installed base relying on the interim transport, so the cutover needs no bridge and no flag-day. Go straight to the API.

The transport is isolated behind a single `fetch_remote()` seam; the model, cache, fallback chain, IPC layer and UI are all transport-neutral. The swap is therefore small. The bulk of the remaining effort — polling lifecycle, wiring the UI, enforcement call sites at finer-than-service granularity — was never finished for the interim transport either, so do not size the work as "just repoint a URL".

## API-side gaps, all verified in code

1. **No audit trail on flag mutations.** `web/src/Helpers/AuditLogger.php` exists with **zero controller call sites**. For a mechanism that disables functionality for legal reasons this is disqualifying — there is no defensible record of who changed what. Fixed first.
2. **No response signing.** Clients trust the transport alone.
3. **No targeting** by version/platform/channel on `main`. A rollout branch carrying **migration 015** is unmerged — settle its disposition before writing migration 016 or the numbers collide.
4. Flag-key sanitiser rejects dots (`^[a-zA-Z0-9_-]+$`), blocking a dot-namespaced scheme across apps.
5. All responses `Cache-Control: no-store`, no `ETag` — every poll is a full round trip.

## Security — the honest version

Three factors (app identifier, User-Agent prefix, hashed key with fail-closed scopes) **are** genuinely enforced on every app-facing route, with generic 403s and auth-failure rate limiting.

But **the key and the User-Agent ship inside every client binary** and are extractable by anyone holding the app. They are attribution and abuse filtering, **not a security boundary**. Never design or document them as though they were.

**Client-side enforcement on a user's own hardware is fundamentally advisory.** A determined user can block the domain, edit hosts, or patch the binary. If a legal obligation requires a *guarantee* that a feature is off, this design cannot deliver it — say so plainly rather than shipping something that reads stronger than it is.

The threat that most deserves design attention is **a compromised API or admin account pushing a malicious flag to the entire installed base**. Containment matters more than perimeter: least-privilege admin roles, staged rollout, instant revert, audit trail, alerting on mass-disable, and a client-side sanity floor that refuses instructions to disable its own fetcher or updater.

**Fail-open is the recommended default.** An unreachable server must never brick an offline app. The cost — that blocking a domain defeats the switch — is real but preferable to breaking every offline user.

**Privacy is part of security here — and the evaluation model changed on 2026-07-27 (see the dated update at the end of this file).** MeedyaDL ships privacy-first commitments (anonymous crash reporting, consent modals, credential redaction); those commitments are preserved not by *where* conditions are evaluated but by *what* the client transmits. The current position: the app identifies its own version, OS type/version and CPU architecture — the same class of information already sent with an update check — and no install identifier, account, locale or settings data is ever sent. See the update below for why this replaced the original "evaluated client-side" position.

**Release-note confidentiality applies with force.** Per `.github/release-notes/STYLE_GUIDE.md`, never disclose the delivery mechanism. A note naming the host hands anyone wanting to defeat the switch the exact thing to block.

## Suite rollout — contract-first, crate-later

- MeedyaDL — Rust + TypeScript. First adopter, reference implementation.
- MeedyaManager — Rust, already consumes shared crates from MeedyaSuite-core.
- MeedyaConverter — **Swift** (SwiftPM, URLSession). Cannot share Rust code.

With only two Rust consumers of three, the **wire contract is the real shared artefact**: language-neutral JSON, signature over raw response bytes, client-side condition evaluation. Extract a shared Rust crate into MeedyaSuite-core only **after** two working Rust implementations exist to factor out — extracting from one bakes in that one's accidents. Keep Rust-shaped assumptions (serde encodings, Rust enum representations) out of the contract; a Swift client must consume it naturally.

`MeedyaSuite` and `Skriptey` orgs were **not in session scope** and were not verified — treat anything about them as plan-only.

MWBM-intAppsAPI had **only a `main` branch**; an `alpha` branch had to be created to match the convention used elsewhere.

## Open decisions (as at 2026-07-27)

1. Is the API deployed and reachable in production? A hosting error was flagged and could not be verified from a session.
2. Ratify fail-open + no-opt-out privacy posture.
3. Disposition of the unmerged rollout branch / migration 015.
4. Signing-key custody — API host, or isolated so a host compromise cannot sign a malicious disable-everything instruction.

## Update — 2026-07-27 (same day, session continued): evaluation model reversed to server-side; DNS + branch findings

**Status:** User-Agent standardisation shipped on MeedyaDL (`6c90ecf5` + `b037560c`, `feat/alpha-consolidated`). The flags client itself is **not yet written**. Full detail and the ordered remaining chain live in `.github/HANDOFF.md` under "Session continued — User-Agent standardisation shipped; remote feature control moves server-side" — this entry records only what changed in this document's own claims.

### The excised sentence, and why

The original text above this update read: *"Conditions are evaluated **client-side** precisely so no install identifier is ever transmitted. A polling client that fingerprinted installs would contradict shipped promises."* That sentence has been removed, not merely superseded, because it is no longer MeedyaDL's design.

**New position: server-side flag evaluation.** The client sends `app_version`, `platform`, and `platform_version`; the server resolves the condition and returns a boolean. The client does not fetch raw rule definitions and evaluate them locally.

**Why this is not a privacy regression:** `full_user_agent()` (landed this session, `utils/http_client.rs`) already sends `"MeedyaDL/{version} ({OSName} {Arch}/{OSVersion})"` to this exact endpoint for every request MeedyaDL makes to it. Server-side evaluation transmits no additional data beyond what the User-Agent header already carries — the privacy delta between the two designs is zero. Client-side evaluation, by contrast, has a real cost: it freezes rule semantics into every already-shipped binary, which defeats the purpose of a remote kill switch (a rule change can't retroactively alter how an old binary interprets a condition it already downloaded).

**The exact replacement privacy wording** (for README/TERMS when the flags client ships, and for any future rewrite of this document): *"the app identifies its own version, OS type/version and CPU architecture — the same class of information sent with an update check — and no install identifier, account, locale or settings data is ever sent."*

This is recorded as **decision B, taken as an assumption — not yet signed off by the maintainer.** Flag it explicitly before the flags client ships to production users.

### Verified findings this session (all in code, not inferred)

1. **`api.mwbmpartners.ltd` has no DNS record.** The apex `mwbmpartners.ltd` resolves via Cloudflare; the `api.` subdomain does not — the API is not deployed anywhere reachable. A client built against it is safe to ship (silent-on-failure, cached, fail-open) but will be inert until the record exists. This does not contradict finding 1 in "Open decisions" above (still genuinely unverifiable whether a deploy exists behind some other, undiscovered hostname) — it narrows it: the *documented* hostname is not live.
2. **No version/platform targeting exists on intAppsAPI `main`.** Migration 015 (percentage / user-allow / user-deny / segment rollout only — no version or platform dimension) lives on unmerged branch `claude/feature-gating-readiness-yQisQ`. Settle its disposition (decision C, below) before writing migration 016, or the migration numbers collide.
3. **Bug on intAppsAPI `main`:** `MigrationRunner`'s discovery regex also matches `*_rollback.sql`, and lexical sort places each rollback immediately after its forward migration — a fresh `migrate.php` run on a clean database applies then immediately rolls back every migration. Fresh installs are broken on `main` today. Fixed on `claude/feature-gating-readiness-yQisQ` (commit `1925be0`).
4. **Bug:** `SemVerComparator` compares the prerelease segment with `strcmp`, so string-sort puts `"alpha.10"` below `"alpha.9"` — every MeedyaDL alpha build past `.9` would be misordered by any version-gated rule. Separately, `normalize()` requires a 3-part version string, but macOS reports `"26.6"`, Ubuntu `"24.04"`, Debian `"12"` (all 2-part) — platform-version rules would silently never match on those OSes. Filed as intAppsAPI #109; must land before #108 (version/platform targeting) or the new feature ships broken on day one.
5. **Branch `claude/feature-gating-readiness-yQisQ` (tip `4b8f2aa`) also carries a `schema.sql` cumulative-snapshot fix (`13e4de0`) and a CI DB Check workflow (`7f8c81e`)** — nothing else in the repo contains those commits, so a rebuild-from-scratch (rejected in favour of decision C) would lose them too. **Merging it into the new consolidated branch now produces a real conflict** in `web/src/Controllers/Admin/FeatureController.php` (both sides independently wire up `AuditLogger`). An earlier session's "zero conflicts" assessment of this branch is **stale and should not be trusted** — re-diff before merging.

### Suite rollout — no change

The contract-first, crate-later position above is unaffected by the evaluation-model reversal: server-side evaluation is still a JSON wire contract any language can consume, so MeedyaConverter (Swift) and MeedyaManager (Rust) are unaffected by which side resolves the condition.

## Update — 2026-07-27 (later same day): client + notice UI shipped; documentation sweep

**Status:** The MeedyaDL client landed as two commits on `feat/alpha-consolidated`: `c4a2185b` ("feat(flags): remote feature availability client with sticky cache and safe defaults" — `services::feature_flag_service`, the three-tier resolution chain, the `UNGATEABLE_KEYS` sanity floor, the sticky no-TTL disk cache, the `get_feature_flags`/`refresh_feature_flags` IPC pair) and `9884e669` ("feat(flags): show an explanation when a feature is temporarily unavailable" — `useFeatureFlagStore`, `selectNoticeEntries`, `FeatureNoticeBanner`, the `featureFlags.fallbackMessage` i18n key, and the piggyback onto the existing update-check `setInterval` in `App.tsx`). Both build on the User-Agent standardisation work referenced in the update above (`full_user_agent()` is this endpoint's UA per the four-way policy in `.claude/CLAUDE.md`).

A documentation sweep followed as three commits: `docs(help)` (Troubleshooting / Supported Services / FAQ, both the `.md` files and their inline `HELP_TOPICS` twins in `HelpViewer.tsx` — `faq` has no inline twin by design), `docs` (README / TERMS / SECURITY / DEV_NOTES / Project_Plan — the DEV_NOTES additions are deliberately limited to the `INTAPPS_*` env-var names + `option_env!()`, nothing about the transport itself), and `docs(claude)` (this file, `.claude/CLAUDE.md`'s new architecture bullet with full technical detail, and a correction to `.claude/analysis/remote-feature-flags-analysis.md` flagging that its evaluation-model description was reversed before implementation — see the update above).

**What is still NOT done** (do not let a future session assume otherwise):

1. **No enforcement call sites exist anywhere in the codebase.** `feature_flag_service::current()`/`is_enabled()`/`notice_for()` resolve and cache verdicts; nothing gates on them yet. Every feature the mechanism could theoretically pause is fully functional regardless of its flag state today. Wiring an actual gate is a separate, not-yet-scoped change.
2. **`notice.url` has no scheme validation and is never rendered.** `FeatureNoticeBanner` explicitly does not render or open it (see the component's own doc comment) — this is a known gap, not an oversight, and must be fixed before any UI surfaces that field.
3. **Server-side evaluation remains an assumption, not a maintainer-ratified decision.** The update above records it as "decision B, taken as an assumption — not yet signed off by the maintainer." That is still true after this shipped client: the client code and this documentation sweep both describe and rely on server-side evaluation (the request carries `app_version`/`platform`/`platform_version`; the server returns resolved booleans), but nobody has gone back to get explicit maintainer sign-off on that architectural choice since it was reversed from the original client-side-evaluation design. Flag this explicitly if the maintainer asks "did we decide this?" — the honest answer is "the code assumes yes, but no one has confirmed it out loud."

## Update — 2026-07-27 (still later same day): enforcement shipped; two defects found and fixed; ungated `spotify:` URI gap identified

**Status:** Enforcement landed as `747c8cda` ("feat(flags): enforce service availability at every enqueue seam") on `feat/alpha-consolidated`, immediately preceded by a fix commit `9042e7d3` ("fix(flags): accept the backend's list-shaped payload and correct the ungateable key grammar") that repaired two defects the previous update's "still NOT done" list didn't yet know existed, because the client that would have exposed them hadn't been pointed at a real payload yet.

**Enforcement itself:** `feature_flag_service::service_gate(app, &MediaServiceId)` (pure half `evaluate_service_gate`) is the one enforcement primitive, resolving through `current()` — sync, never network, so an offline user pays nothing for it. It is wired at exactly four enqueue seams: `start_download`, `retry_download`, `retry_failed_bulk`, `import_queue` (the last one per-item — a blocked item is skipped and counted rather than failing the whole import, since an import file is often an old, multi-service archive). It is deliberately absent from `process_queue`, startup queue recovery, `try_fallback`, gap-fill retries, companion downloads, the enrichment pipeline, and `retry_download_without_wrapper` (the automatic continuation of an already-admitted download's failure, not a fresh admission). That split is the entire safety property: a pause stops new work starting and is never retroactive — nothing already in flight is ever strandable by a flag flip. The dead `service_dispatch::is_service_remotely_enabled()`/`service_disabled_error()` pair (zero call sites, read the superseded `service_status` transport) was deleted rather than left in place, specifically so a future implementer couldn't wire a new gate to the wrong backend by finding them via autocomplete.

**Defect 1 — the wire shape never matched, and the silent-failure contract hid that permanently.** `FeatureFlagsSnapshot::verdicts` was typed as `HashMap<String, FlagVerdict>` with only `alias = "features"`. The live server's `FeatureController::list()` actually answers with `data.features` as a JSON **array** of `{feature_key, enabled, ...}` objects — serde cannot deserialize an array into a map, so every real fetch failed to parse. Because `refresh()`'s whole design is "on any failure, log exactly one generic line and change nothing visible," this defect was invisible by construction: it degraded to compiled defaults (everything enabled) forever, with no error surfaced anywhere a human would look. It was caught only because someone finally pointed the client at a real response and thought to check what actually landed in `verdicts`, not because any test or log line complained. Fixed with a custom `deserialize_with` (`VerdictsWire`, untagged) that accepts either shape: object as-is, or an array folded into a map keyed by `feature_key` with empty/missing keys dropped. Serialization — and therefore the on-disk cache format — is unchanged; only the read path grew tolerant.

**Defect 2 — the containment the sanity floor was built for did not actually hold.** `UNGATEABLE_KEYS` used dotted keys (`"core.remote-config"`, `"core.updater"`), but the backend's `InputSanitizer::slug()` enforces `^[a-zA-Z0-9_-]+$` and rejects dots outright — the server can never create or serve those exact strings. `apply_sanity_floor()` only forces `enabled = true` on keys **present** in a payload; it does nothing for a key that's simply absent. So a malicious payload disabling `"core-updater"` (the only spelling the server could actually send) would never have matched the floor's dotted entry — the documented promise "a compromised admin account cannot blind the fleet" was aspirational, not actually enforced, from the moment `UNGATEABLE_KEYS` was written until this fix. Renamed to kebab-case (`"core-remote-config"`, `"core-updater"`) with every doc-comment example and test updated to match. Because defect 1 meant no shipped build had ever successfully parsed a real payload, this rename carries no on-disk or in-flight compatibility burden — there is no data anywhere using the old spelling.

**Kebab-case is now the whole key taxonomy, not just the two ungateable keys.** Every flag key — `MediaServiceId::flag_key()`'s five outputs (`service-apple-music`, `service-youtube-music`, `service-youtube`, `service-spotify`, `service-bbc-iplayer`) included — matches `^[a-z0-9-]+$`, namespaced by prefix (`core-`, `service-`, `feature-`) rather than by dot. Any future flag key, anywhere in this system, must follow that grammar or the server will silently be unable to serve it — the exact shape of defect 2.

**Server-side evaluation is now load-bearing, and is still unratified.** The prior update flagged server-side evaluation as "decision B, taken as an assumption — not yet signed off by the maintainer." That was true when it only affected notice display. It is materially more true now: enforcement means a server-side evaluation bug (the `SemVerComparator` `strcmp` bug and the 2-part-OS-version bug on intAppsAPI, both still open per the "Open decisions" update above) doesn't just mis-render a banner — it can incorrectly refuse or incorrectly admit real downloads. Get explicit maintainer sign-off on the server-side-evaluation architecture before intAppsAPI's version/platform targeting (the still-unmerged `claude/feature-gating-readiness-yQisQ` branch) ships, not after.

**Gap identified, not fixed: a bare `spotify:` URI reaches the queue completely ungated.** `start_download`'s classification loop (`commands/gamdl.rs`, immediately after the `SUPPORTED_HOSTS` allowlist) only inspects `url::Url::parse(url)` results whose `scheme()` is `"http"` or `"https"`:
```rust
if parsed.scheme() == "http" || parsed.scheme() == "https" {
    // host allowlist check, has_spotify / has_apple_music classification
}
```
A URI using the `spotify:` scheme (e.g. `spotify:track:...`) parses successfully but has a scheme of `"spotify"`, not `"http"`/`"https"` — so it never enters that `if` block. It is never checked against `SUPPORTED_HOSTS`, never sets `has_spotify`, never contributes to `batch_services` for the feature-availability gate, and never trips the M9 Spotify anti-ban dispatch gate a few lines later (which only fires `if has_spotify`). If nothing downstream separately rejects non-`http(s)` schemes, such a URL can reach `q.enqueue()` having passed through zero of the checks — domain allowlist, feature-availability pause, and Spotify consent/dev-access/DLL/daily-cap gate — that every `open.spotify.com` URL is subject to. This was found by re-reading the classification loop while documenting enforcement, not by exploiting it; it has not been reproduced end-to-end or fixed. File as a follow-up before Spotify (M9) ships broadly: either extend the classification loop to also recognise the `spotify:` URI scheme, or reject any non-`http(s)` scheme outright at the top of `start_download`.

## Update — 2026-07-27 (final checkpoint this leg): bare `spotify:` gap fixed; percentage-rollout fail-open fixed on the API side; four-URL-form deployment; still never run against a live server

**Status:** This is the closing entry for the leg that opened with the enforcement update immediately above. One MeedyaDL commit and, separately, four MWBM-intAppsAPI commits (its own consolidated branch, `feat/feature-targeting-consolidated` — one branch per repo, no PR stacking, the same rule this repo follows) landed. Nothing here is a plan; it is a record of what shipped and what is still open.

### 1. The bare `spotify:` URI bypass — fixed

`b5924ae5` ("fix(flags): classify bare spotify: URIs so they pass the enqueue gates") closes exactly the gap the previous update identified and left unfixed. The classification `if` genuinely had no `else` — a `spotify:album:...`/`spotify:track:...` URI fell through with no host check, no `has_spotify` flip, and **no rejection of any kind**. Restating the blast radius precisely, because it is worse than "ungated": it evaded the remote feature-availability gate **and** the entire M9 Spotify anti-ban dispatch gate — dev-access, consent, DLL/`.wvd` presence, daily download cap — all of it, simultaneously. Worse still, `MediaServiceId::from_url()` also only recognises `open.spotify.com`, so the item enqueued with `service: None`, and `process_queue()`'s legacy fallback treats `service: None` as **Apple Music** — meaning a Spotify URI was actively dispatched to GAMDL, which rejected it with an error naming neither Spotify nor the actual cause. A user hitting this would have had no way to self-diagnose it.

Fix shape: classification was factored out of the inline `if` into a pure, unit-tested `classify_batch_urls()` helper with an explicit `else if parsed.scheme() == "spotify"` branch that sets `has_spotify` — so a bare `spotify:` URI is now classified at the exact same point, and subject to the exact same two gates, as an `open.spotify.com` link. A second helper, `reject_bare_spotify_uris()`, runs immediately **after** both gates (not before, and not instead of them) and explicitly rejects the bare-URI shape with a message naming the real cause and pointing at the supported `open.spotify.com` form. The ordering is deliberate: a paused Spotify service, or a blocked dispatch-gate outcome, must still surface *that* message first — a generic "unsupported URL scheme" message would be a worse UX regression even though it would also technically block the request. Neither GAMDL's nor votify's subprocess builder accepts anything but `http(s)` URLs today, so the scheme is now recognised for gating purposes but the bare-URI shape is still not functionally routable — that is unchanged and is not a defect, just a scope boundary. 9 new unit tests cover the classification, the mixed-batch case, the routability guard's message content, an end-to-end proof that a disabled Spotify flag refuses a batch containing a bare `spotify:` URI, and three regressions confirming unparseable / unsupported-host / unrelated-scheme URLs are handled exactly as before.

### 2. Percentage rollout failing closed — fixed on the API side

`24ed917b` ("fix(features): percentage rollout no longer disables clients it cannot bucket") fixes a defect independently discovered and documented on the intAppsAPI side: `Feature::evaluateRollout()` returned `false` for any caller without `user_id`, **before** consulting the configured percentage at all. Desktop clients — MeedyaDL, MeedyaConverter, MeedyaManager — never send `user_id` (by design; see the client-transmission wording earlier in this file), so an operator setting a 50% rollout was silently removing the feature from **100%** of every desktop install, the opposite of what "50%" means to the person configuring it. No error, no failing test, the admin UI and audit log both looked correct — this is the same class of invisible failure as defect 1 in the enforcement update above (a defect that degrades to a plausible-looking default with nothing to catch it).

Percentage was the *sole* outlier in the rollout-strategy code: deny-list, allow-list, and segment checks are each guarded by a presence test for their needed context and already fall through to enabled when that context is absent — i.e. they were already fail-open. The fix makes percentage follow the identical rule. Recorded explicitly as a **decision, not an oversight**: an allow-list-only strategy also keeps failing open on a missing `user_id`, because an allow-list states "these callers are IN," not "everyone else is explicitly OUT" (that requires a deny-list or a disable rule) — so it is treated the same as every other dimension rather than carved out as a special case. Net practical effect, now documented in the admin guide text and intAppsAPI's own CLAUDE.md: **percentage rollout is currently a no-op for every desktop suite app** — an operator wanting partial rollout to these apps must use a disable rule instead.

This is relevant to MeedyaDL's threat model above even though it is API-side code: it means that for the lifetime of the bug, any percentage-based rollout configured against MeedyaDL's flag keys would have behaved as a 100%-disable, not a partial rollout — a footgun in the opposite direction from the "compromised admin blinds the fleet" threat this file's Security section focuses on, but with the same practical outcome (a flag operator's intent silently not matching what shipped).

### 3. Four URL forms, one deployment (verified on the sibling repo)

`0f70813` + `b057b80` moved the API's deployment shape: `service.api.<domain>/` serves the app at the root, and `api.<domain>/service/` serves the identical files under a `/service/` path segment — one pair of forms per brand (`meedyasuite.com` for Meedya-branded apps including MeedyaDL, `mwbm.io` for the rest), both served from a single checkout. `web/.htaccess`'s `RewriteBase` is now **deliberately absent** — a hardcoded `/service/` value satisfies only the path-segment form; the relative substitution under the subdomain form expands to a nonexistent `/service/index.php`. The previous value's original rationale (avoiding Apache's per-directory base-guessing misresolving through a symlinked directory) is real but narrower than what it broke, and is preserved as a documented in-file fallback rather than deleted outright. **Unverified against the real host either way** — Apache's per-directory base inference through a symlink is exactly the kind of thing that only shows up empirically.

### 4. Still never run against a live server

Unchanged from every prior update in this file: the wire shape is correct against the API source and parses in unit tests, but no request from either app has ever reached a running instance of the API. Because `refresh()`'s failure path is silent by design (see the Silent-failure contract in `.claude/CLAUDE.md`'s "Remote feature availability" bullet), a remaining mismatch on first real contact will present as **nothing happening** — not a visible error, not a crash, not a toast. The diagnostic signal is the Activity Log line `"Feature availability refresh failed — keeping last known status"`, not the absence of an error message. This remains the single largest unverified assumption in the entire programme and should be the first thing checked once the API is actually deployed and reachable (DNS + TLS + provisioning are still entirely outstanding per `.github/HANDOFF.md`'s final checkpoint).

### Open, unchanged or newly recorded

- Server-side flag evaluation (decision B) is **still unratified** by the maintainer and is now load-bearing for two things, not one: enforcement (previous update) and, as of finding 2 above, the percentage-rollout semantics an operator now has to understand correctly to avoid a silent 100%-disable.
- Canonical URL form per brand (finding 3) — each app's build secret must hold exactly one of the two working forms; not yet chosen.
- Whether `api.mwbmpartners.ltd` retires or redirects to the new hosts — currently treated as plain replacement, unconfirmed.
- A suite client-integration doc for MeedyaConverter/MeedyaManager/CueRCode/Go2My.Link belongs in the private IntAppsAPI repo and does not exist yet; those four repos are attached to sessions but have no feature-availability work started on `main`.
- `notice.url` is still parsed but never rendered — still needs scheme validation before it can be.
- `Feature::applySchedules()`'s cache invalidation (`deletePattern('features:*')`) does not reach `feature_rules:app:*` — harmless while a schedule flip changes no rules, filed as a trap for whenever that stops being true.
- The dormant interim `service_status` transport (model, IPC command, store, banner) is superseded by everything documented in this file and should be removed rather than left as a second, dead code path.

## Update — 2026-07-28: canonical URL form per brand — DECIDED

**The decision.** Every first-party app connects to the MWBM-IntAppsAPI service API using the **subdomain** convention:

```
https://service.api.<domain>/
```

**not** the path convention (`https://api.<domain>/service/`), which remains available and serves the identical files but is no longer what apps are built against. This resolves the "canonical URL form per brand" item that the previous update ("Findings worth preserving from this leg", finding 3) left open ("each app's build secret must hold exactly one of the two working forms; not yet chosen").

**Brand mapping (concrete hostnames — permitted in this file):**

- Meedya-branded apps (MeedyaDL, MeedyaConverter, MeedyaManager, MeedyaPlayer, MeedyaSubtitler) → `https://service.api.meedyasuite.com/`
- All other apps (CueRCode, Go2My.Link, etc.) → `https://service.api.mwbm.io/`

MeedyaDL takes `https://service.api.meedyasuite.com/` as its `INTAPPS_BASE_URL` build secret because it is a Meedya-branded app.

**Why this matters technically, not just which string to paste in.** Under the subdomain form the app is served at the **domain root**; under the path form the identical files sit under a `/service/` segment. `web/.htaccess` deliberately has **no `RewriteBase`** so Apache infers the base from the directory holding the file — that inference is correct at either mount depth, which is exactly why the file can serve both forms from one checkout without a config branch. The practical consequence of this decision is that the root-mounted (subdomain) form is now the **primary** case client applications actually run under, and the documented symlink caveat already on that `.htaccess` (Apache's per-directory base-guessing potentially mis-resolving through a symlinked directory) only ever affected the path-mounted form — which this decision demotes from "the form apps use" to "a secondary, still-served, still-provisioned form for browser/admin access." Nothing about the `.htaccess`'s own mechanism changes; what changes is which of the two forms is now load-bearing for the fleet of client apps versus which is incidental.

**No application code changes.** Each client's base URL is injected at build time — `option_env!("INTAPPS_BASE_URL")` in MeedyaDL, the same pattern for the other Rust/Swift clients per the contract-first suite rollout above — so this decision is a **build-secret VALUE**, not a code edit. Nothing in `feature_flag_service.rs` or its call sites needs to change; only the string set in the `INTAPPS_BASE_URL` GitHub Actions secret (per app repo) needs to reflect the chosen hostname.

**Status of the four hostnames.** All four (`service.api.meedyasuite.com`, `api.meedyasuite.com`, `service.api.mwbm.io`, `api.mwbm.io`) still need DNS records and TLS certificates — the path form is not being retired, only de-primaried. See `.github/HANDOFF.md`'s provisioning notes for the full outstanding-work list (DNS, TLS, PHP version selection, `user_agent_prefix` registration, key minting, per-repo `INTAPPS_*` secrets).
