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
