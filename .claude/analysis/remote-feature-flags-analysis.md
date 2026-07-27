# Remote Feature Flags — Cross-Repo Analysis & Implementation Plan

**Date:** 2026-07-27 · **Author:** Claude (deep-analysis session)
**Repos analysed:** `/home/user/MeedyaDL` (at `origin/alpha` = `243e8a2a`), `/workspace/mwbm-intappsapi` (`main` = `6816ed8`, shallow clone depth 1), `/workspace/meedyaconverter` (`main` = `be62e6b`), `/workspace/meedyamanager` (`main` = `aa0ad24`).
**Method:** every claim below was verified by reading the named file at the named revision. Where something could not be verified, that is stated explicitly.

**Framing note (per project direction):** the `raw.githubusercontent.com/…/service-status.json` mechanism in MeedyaDL was a *deliberate interim transport*, adopted knowingly while intAppsAPI was built out. Nothing below treats it as a defect. The question answered here is: **is the API ready to take over, and what is the cleanest cutover?**

---

## Part 1a — MeedyaDL current state (verified at `origin/alpha`)

### Files and data model

| File | Status |
|---|---|
| `src-tauri/src/services/service_status.rs` (234 lines) | Compiled, registered, **live** as a module |
| `src-tauri/src/models/service_status.rs` | Compiled, live |
| `src-tauri/src/commands/service_status.rs` | `check_service_status` **is registered** in `lib.rs:1277` `generate_handler![]` |
| `src-tauri/src/services/service_dispatch.rs` | Contains `is_service_remotely_enabled()` + `service_disabled_error()` — **zero callers** outside the module (grep-verified) |
| `src/stores/serviceStatusStore.ts` | **`@ts-nocheck`, dormant** — imports `checkServiceStatus` from `tauri-commands.ts`, which does **not** exist there (grep-verified) |
| `src/components/common/ServiceStatusBanner.tsx` | **`@ts-nocheck`, dormant** — not imported by `MainLayout.tsx` or anything else |
| `src/types` | No `ServiceStatusConfig` TS type exists on `origin/alpha` (grep-verified) |

Data model (`models/service_status.rs`):

```
ServiceStatusConfig {
  version: u32,                                  // schema gate; only 1 accepted
  updated_at: String,                            // ISO 8601, informational only
  services: HashMap<String, ServiceStatusEntry>, // keys "AppleMusic" | "YouTube" | "YouTubeMusic"* | "BBCiPlayer" | "Spotify"
  global_message: Option<String>,                // app-wide announcement
}
ServiceStatusEntry { enabled: bool, message: Option<String> }
```

\* `service_id_to_key()` maps `MediaServiceId::YouTubeMusic → "YouTubeMusic"`, but `all_enabled()` seeds only 4 keys (no YouTubeMusic) and the committed JSON has only 4 services. Harmless today (missing key = enabled) but an inconsistency to tidy at cutover.

### Fetch / poll lifecycle — what actually runs

- **Fetch:** `fetch_remote()` GETs the hard-coded const `SERVICE_STATUS_URL` (raw.githubusercontent.com …/main/service-status.json) with a 10 s timeout via the shared `utils/http_client::build_simple()`, plus a manual `User-Agent: MeedyaDL` header. Rejects non-2xx and `version != 1`.
- **Fallback chain:** remote → local cache (`{app_data}/service-status-cache.json`) → `ServiceStatusConfig::all_enabled()`. Pure **fail-open** at every tier; missing service key also fail-open.
- **Cache write:** plain `std::fs::write` — **not** the repo's `atomic_write_json` helper. Minor gap vs. the repo's own atomic-writes convention; fix at cutover.
- **Polling:** the "checked on launch and every 4 hours" described in the module comments **does not exist**. No timer, no startup call — `checkStatus()` lives only in the dormant store. **Nothing in the running app ever invokes `check_service_status`.**
- **Offline behaviour:** would be cache-then-default; currently moot (see below).

### What a "disabled" verdict does today

**Nothing.** The enforcement helpers exist (`service_dispatch::is_service_remotely_enabled`, `service_disabled_error`) but have no call sites in `start_download`, the queue, or anywhere else. Granularity of the *model* is per-media-service only (5 services). No engine-, stage-, or feature-level gating is expressible. The `.claude/memory/project_multi_service_groundwork.md` file confirms this was staged groundwork for M8–M10, not live plumbing.

### Integrity / authentication of the payload

- Transport: TLS to `raw.githubusercontent.com`. No auth (world-readable public repo file — intended, it's a broadcast). No payload signature, no pinning.
- Trust anchor today = *GitHub org account security + TLS*. Anyone with push access to `MWBMPartners/MeedyaDL@main` controls the switch; the git history is, incidentally, a decent audit trail — a property the API replacement must not lose.
- A user can defeat it by blocking the domain (which also breaks GitHub-based update checks). Fail-open means blocked = everything enabled.

### Does `service-status.json` exist on `main`?

**No.** It exists on `origin/alpha` (committed in `4545dcae`, PR #899, all four services `enabled: true`, `global_message: null`) and in the worktree — but **not on `origin/main`**, which is the branch the hard-coded URL points at. So today the remote fetch 404s and every install would resolve to cache-or-default. Consistent with "interim mechanism, staged but never promoted": **even the interim transport is not actually operative in production right now.** (Action regardless of the API cutover: either promote the file to `main` or accept the default path as the status quo.)

### Seam quality — was the swap designed for?

**Yes, largely.** The transport is isolated behind a single function (`fetch_remote()`); everything else (cache, fallback, `is_service_disabled`, IPC command, store, banner) consumes the transport-neutral `ServiceStatusConfig`. Pointing the fetch at intAppsAPI and mapping its response into `ServiceStatusConfig` is a **contained, low-risk change** — one file materially, plus new response-mapping code.

Where the interim assumption *did* leak, mildly:
1. The **payload shape is the model** — `services` map keyed by PascalCase names, `version`, `global_message` are artefacts of a flat static file, and the frontend store hard-codes the PascalCase key map (`SERVICE_KEY_MAP`). Fine to keep as an *evaluated view*, but per-service booleans cannot express finer-grained gating; richer gating needs a new flags layer beneath (Part 3).
2. `updated_at`/`version` semantics assume a single global document rather than per-flag records.
3. The dormant frontend was written against an IPC wrapper that was never added, so the frontend side is a (small) build-out, not a swap.

**Effort verdict:** transport cutover ≈ small (1–2 commits). Making the mechanism actually *do* something (polling, banner, enforcement) is the larger, independent piece that was never finished for the interim transport either — budget most of the MeedyaDL work there.

---

## Part 1b — intAppsAPI current state (verified at `main` = `6816ed8`)

**Caveat:** shallow clone (depth 1); history-dependent claims come from the repo's own `.claude/CLAUDE.md` (last updated 2026-07-21) and are marked as such. Only `main` exists as a branch — **no `alpha` branch** (creating one is a setup step, per project direction). Note: the repo memory says main was frozen at `7876cef` with PR #102 open; the clone shows `6816ed8` (PR #103 merged) and PR #102's hardening (fail-closed scope enforcement in `AuthMiddleware::permissionAllowsMethod`) **is present in the code**, so #102 evidently merged after that memory was written.

### Feature-flag data model (`web/sql/schema.sql`, `web/src/Models/Feature.php`)

`tblAppFeatures`: `feature_id` PK, `app_id` FK→`tblApps` (CASCADE), `feature_key VARCHAR(100)`, `feature_label VARCHAR(200) NULL`, `is_enabled TINYINT(1) DEFAULT 1`, `enabled_at`/`disabled_at DATETIME NULL` (scheduling, migration 006), `metadata_json JSON NULL`, timestamps. `UNIQUE(app_id, feature_key)`, index on schedule columns.

- **Per-app scoping: yes** (by `app_id`, resolved from URL `{app_slug}` cross-checked against the authenticated app — `ResolvesApp` trait).
- **Targeting by version/platform/channel: NO** on `main`. Repo memory describes an unmerged branch `claude/feature-gating-readiness-yQisQ` with `rollout_strategy` JSON + migration **015** — *not verifiable in this shallow clone*; treat as pending integration.
- **Unknown flag default:** single-key GET returns **404**; batch/list simply omit unknown keys. The server has no opinion — the client's compiled-in default governs (good; keep it that way).
- **Effective state:** `Feature::isEffectivelyEnabled()` folds schedule times in at read time. Note: `Feature::applySchedules()` (which flips the DB rows) has **no caller anywhere** in `web/` — scheduling works only via the lazy read-time evaluation; the batch DB-flip is dead code on `main`.
- **Caching:** optional Redis, TTL 300 s, silent degradation (`Cache` helper); invalidated on mutation.

### HTTP contract a client must satisfy (verified in `AuthMiddleware.php`, `routes.php`, `Response.php`, `HmacValidator.php`)

- **Every app-facing route** goes through the `$appAuth`/`$crashAuth`/`$emailAuth` stacks, each ending in `AuthMiddleware`: `/v1/heartbeat`, `/v1/features/{app_slug}` (+ `/{feature_key}`, `POST …/batch`), `/v1/notifications/{app_slug}`, `/v1/updates/{app_slug}[/{channel}]`, `POST /v1/crash-reports/{app_slug}`, `POST /v1/email/{app_slug}/send`. **No unauthenticated app endpoint exists** (only `/`, `/v1/status`, `/v1/health`, `/docs`).
- **Three factors, all enforced:** (1) `X-App-ID` = active app UUID; (2) `User-Agent` must `str_starts_with(user_agent_prefix)` — skipped only if prefix is empty string; (3) `X-API-Key` bcrypt-verified against scoped keys (`tblAppApiKeys`, migration 004: label, permissions JSON, `is_active`, `last_used_at`) first, then the legacy app-level `api_key_hash`. Scoped keys are **fail-closed** (`permissionAllowsMethod`: empty/unknown scopes permit nothing; read/write/full). Legacy key = full access.
- **HMAC** (`X-Signature` + `X-Timestamp`) required **only on POST/PATCH/DELETE**: `hex(hmac_sha256(rawBody + "." + timestamp, secret))`, timestamp window ±300 s (`Constants::HMAC_MAX_AGE`, overridable ≥10 s). GET feature reads are *not* HMAC-signed — TLS + 3-factor only.
- **Failure handling:** any factor mismatch → generic 403 (`ApiException::accessDenied()`, no detail), failed attempt recorded in `tblRateLimits` (`app_auth_fail`, 10/min per app-ID+IP → 429). Scope failures log a warning with app slug + method.
- **Rate limits:** `app` group 60/min, `crash` 10/min (`web/config/security.php`).
- **Response envelope:** `{"success": true, "data": …}` / `{"success": false, "error": {"code", "message"}}`; headers include `X-Request-Id`, and **`Cache-Control: no-store` on every JSON response — no ETag/If-None-Match anywhere** (grep-verified). Polling clients re-download the full payload each time (payloads are small; still worth adding conditional GET — Part 4).
- **Feature response item:** `{feature_key, label, enabled (effective), enabled_at, disabled_at, metadata}`.
- **No response/payload signing exists anywhere** (grep-verified). Client-side integrity = TLS only.
- **Key constraint that shapes the suite-wide namespace:** `InputSanitizer::slug()` enforces `^[a-zA-Z0-9_-]+$` (max 100) on `feature_key` — **dots are rejected**. Either the namespace avoids dots or the sanitiser gains a variant (recommended, Part 4/5).

### Admin UI (verified `web/admin/assets/js/components/features.js`)

Can: list per app, client-side search, toggle `is_enabled` (admin role), create/edit/delete, set `enabled_at`/`disabled_at` (datetime-local), edit raw `metadata_json`. Roles: `admin` vs `viewer` (`AdminRoleMiddleware`). Cannot: targeting/rollout, flag classes, user-facing message as a first-class field, change-history view for a flag, bulk operations, "confirm before kill-switch" friction.

**Material gap:** `AuditLogger` (`web/src/Helpers/AuditLogger.php`, migration 005, admin audit-log routes) **is never called from any controller** (grep-verified: only its own file references it). Flag mutations are captured only by the generic request-level `ActivityLogMiddleware`. For a mechanism whose whole point is "who disabled what, when, why", structured audit wiring is required work, not polish. (Repo memory says the unmerged branch B wires feature webhook/audit — unverifiable here.)

### Migration & test conventions

- Migrations: `web/sql/migrations/NNN_name.sql` (+ `NNN_name_rollback.sql` since 008), applied by `web/migrate.php` (`--rollback[=N]` supported). `main` has 001–014; **next free number is 015 — but repo memory says unmerged branch B already uses 015**. Recommendation: integrate branch B first (the repo's own 2026-07-21 audit recommends exactly that), else take **016** to avoid collision.
- Tests: `web/tests/Unit/{Core,Helpers}` (5 files: ApiException, HmacValidator, InputSanitizer, ResolveSafePublicIp, SemVerComparator); **`web/tests/Integration/` is empty**. PHPUnit config at `web/phpunit.xml`; CI (`.github/workflows/ci.yml`) runs `vendor/bin/phpunit`.
- Conventions (`.claude/CLAUDE.md`, `DEV_NOTES.md`, `README.md`): PHP 8.5-target/8.4-compatible; **MySQLi prepared statements only**, `tbl` prefix; no runtime Composer packages; Keep-a-Changelog + SemVer (currently v0.3.0, v1.0.0 never cut); vanilla-JS admin SPA, 4 theme modes; **no auto-push, PRs only when asked**; branch protection on `main` requires checks named `Backend (ubuntu-latest)` + `Frontend (ubuntu-latest)` byte-for-byte.
- **Deployment status: NOT VERIFIED.** Whether `https://api.mwbmpartners.ltd` is live could not be checked from this session. Repo memory records two deploy blockers: `web/.htaccess` uses `<DirectoryMatch>` (illegal in `.htaccess` → 500s on DreamHost) and PHP 8.4 availability on the host. **The API being deployed and reachable is a hard prerequisite for the entire cutover** — settled by an ops check, not by code reading.

---

## Part 2 — Gap analysis

### Field-by-field: what carries over from the interim payload, what changes shape, what is new

| Interim field | Verdict | In the API |
|---|---|---|
| `version` (doc schema) | **Drop** — artefact of a single static file. API versioning lives in `/v1` + envelope. Client keeps a defensive parse guard. |
| `updated_at` (global) | **Restructure** — becomes per-flag `updated_at` (column already exists, not exposed) + a response-level `generated_at`. Worth exposing for staleness display. |
| `services{PascalCase → entry}` | **Restructure** — artefact of a flat file. Becomes namespaced flags in the flat per-app feature list (`service-apple-music` etc.). Client maps keys; PascalCase dies at the seam. |
| `enabled` | **Carries over** — `is_enabled` ⊕ schedule = effective `enabled` (richer already: scheduling). |
| `message` | **Carries over, promoted** — becomes structured client-facing metadata (see Part 4: `client_message`, `severity`, `docs_url`, `fail_policy` under a documented `metadata` schema). |
| `global_message` | **Migrates to a different primitive** — the existing `/v1/notifications/{app_slug}` endpoint (severity + version targeting per README) is the right carrier for announcements; don't re-express as a flag. |
| *(inexpressible before)* | **New:** per-app scoping, scheduling, per-flag audit trail, analytics (`tblFeatureAnalytics`), scoped/rotatable credentials, admin UI, webhooks — plus **to build:** version/platform/channel targeting, staged rollout, flag classes, payload signing, conditional GET. |

### Needs-matrix: MeedyaDL gateable surface vs. API expressiveness

| Gate dimension (from MeedyaDL's CLAUDE.md surface) | Expressible today? | API-side work | MeedyaDL-side work |
|---|---|---|---|
| Whole app ("update required" hard notice) | Partially (flag + notification) | Flag class + client contract | Startup gate + banner |
| Per media service (AppleMusic/Spotify/YT/YTM/BBC) | **Yes** — one flag per service | Naming convention only | Enforcement call sites (exist, uncalled) + banner wiring |
| Per engine (gamdl/votify/yt-dlp/get_iplayer) | **Yes** — one flag per engine | Naming convention | New gate in `engine_registry`/`resolve_engine_chain()` |
| Per engine **version window** (e.g. "gamdl ≤3.8.1 broken") | **No** | Targeting/conditions JSON (client-evaluated) | Evaluate condition against `gamdl_capabilities` cached version |
| Per enrichment stage (12 stages) | **Yes** — flag per stage | Naming convention | Per-stage check in the enrichment pipeline (shutdown-signal-style iteration checks already exist as the pattern) |
| Premium/token features (syllable lyrics, animated artwork, MV relations) | **Yes** | Naming convention | Gate inside `resolve_premium_feature_token()` / stage entries |
| Update channels | **Partially** — `/v1/updates` endpoints exist but MeedyaDL's updater is GitHub-Releases-native | Out of scope now | Out of scope now (note as future) |
| Crash/error reporting paths | `/v1/crash-reports` exists | Out of scope now (GitHub Issues + optional Sentry already shipped) | Optional later |
| App version / platform / channel **targeting** of any flag | **No** on `main` | **Core new work** (targeting JSON; branch B is a head start) | Client-side evaluator |
| Staged/percentage rollout | **No** on `main` | New work (deliberately simple: percentage + deterministic client-side coin toss — see privacy note) | Client-side bucket |
| Payload integrity beyond TLS | **No** | Ed25519 response signing | Embedded public key + verify |

### The hard parts

**Fail-open vs fail-closed.** Recommendation, per flag class (make the class an explicit server-side field so policy is data, not code):

| Class | Unknown/missing flag | API unreachable | Cached copy exists |
|---|---|---|---|
| `kill_switch` (service/engine/stage disable) | Enabled (fail-open) | Use last-known-good cache **with no expiry** ("sticky") | Honour cache |
| `operational` (tuning, e.g. retry budgets via metadata) | Compiled-in default | Compiled-in default or cache | Honour cache |
| `experiment` (staged features) | Compiled-in default (usually off) | Cache, else default | Honour cache |

Rationale: a privacy-first desktop app must work fully offline indefinitely — fail-closed would brick air-gapped/firewalled users and is therefore off the table for anything user-facing. The corollary must be stated honestly: **a user who blocks the API hostname permanently defeats the kill switch.** Sticky last-known-good caching means a flag seen once keeps applying offline, which is the strongest defensible posture: the switch degrades from "guaranteed" to "reaches every install that ever comes online again". Do not design or describe it as more than that.

**Trust.** Threat model in Part 4. Summary of the recommendation: keep TLS as the transport floor; add an **Ed25519 detached signature over the exact raw response body bytes** (key held server-side in env, public key embedded in clients). This protects against TLS-interception middleboxes, CDN/hosting-layer tampering, and — partially — web-tier compromise (only if the signing key is segregated from the web runtime; on shared hosting it won't be, so label it defence-in-depth, not a boundary). Blast-radius containment for the *self-inflicted / compromised-admin* case matters more: audit trail, mass-disable alerting, staged rollout, instant revert, and a client sanity floor (client ignores any instruction that would disable its own flag-fetching or its updater — those two capabilities are not gateable, by contract).

**Caching and staleness.** Poll on startup + every 6 h (mirrors the update-check cadence pattern already in `App.tsx` Effect 4). Cache persisted across restarts via `atomic_write_json`. Flag flips are enforced **at enqueue/preflight, never mid-download** — aborting an active GAMDL subprocess on a poll tick invites data corruption and support tickets; the queue's existing preflight-gate architecture (three gates in `DownloadForm.tsx` + queue pre-flight) is the natural insertion point. Expected worst-case propagation: ≤6 h for running apps, next-launch for closed ones — state this in the admin UI so operators have correct expectations.

**Privacy.** Minimum the client must send: `X-App-ID` (per-**app** UUID, identical for every install — deliberately not an install identifier), `X-API-Key` (per-app, identical across installs), `User-Agent: MeedyaDL/<version> (<os>)`. That's it. **No install UUID, no locale, no settings fingerprint, ever.** Version+platform in the UA is justified: it is required for the User-Agent auth factor anyway, matches what MeedyaDL already reveals to GitHub on every update check, and enables *server-side log analytics* without any structured per-install field. Targeting is **evaluated client-side** (server returns the conditions; client applies them) precisely so the client never has to post its configuration upstream. Rollout percentages use a locally-generated persisted random bucket number that is *never transmitted*. Server-side: IPs land in `tblActivityLog` (schema stores `ip_address`, `user_agent`) — retention policies exist (migration 009); confirm the retention window covers this and document it in MeedyaDL's privacy story.

**Migration (planned handover, no flag-day).** Three-phase:
1. **Bridge:** promote `service-status.json` to `MeedyaDL@main` (it 404s today), and add a small generator so the file is *produced from the API's flag state* (admin export endpoint or CI job) — one source of truth, two transports.
2. **Cutover:** new MeedyaDL releases fetch API-first → static-JSON-second → cache → default. The static file stays in the chain as a degraded fallback for exactly as long as it stays published.
3. **Sunset:** already-installed versions only know the old mechanism — but since **no shipped MeedyaDL version ever polled or enforced anything** (Part 1a), there is no legacy population to serve. The bridge file is therefore optional; keep it only if pre-cutover releases will ship with the interim mechanism activated. If not, skip Phase 1 entirely and cut over directly. **Recommendation: skip the bridge; go API-first in the next minor.**

---

## Part 3 — Implementation plan, MeedyaDL

Ordered, one PR (repo standing rule: no PR stacking; this all lands as one PR to `alpha` with per-commit `Release-Note:` trailers, rebase-merge eligible only if every commit carries one).

**Security posture for this repo (threat-model summary; full model in Part 4):** the client's job is (a) verify what it fetched (signature, schema), (b) never send more than the Part 2 privacy minimum, (c) fail open per flag class, (d) refuse self-disabling instructions, (e) never log credentials (the API key must go through the same redaction discipline as `redact_url_query()` does for wrapper URLs).

**Release-note confidentiality (flagged as required):** per `.github/release-notes/STYLE_GUIDE.md`, no commit or release note may name the API host, endpoints, headers, keys, polling cadence, or transport at all. Admissible vocabulary is UI-visible only: e.g. `Release-Note: MeedyaDL can now tell you when a music service is temporarily unavailable, and why.` A note that says "remote kill switch via api.mwbmpartners.ltd" would hand an adversary the exact hostname to block — treat this as a review gate on every commit in the PR.

**Commit 1 — `feat(flags): remote flag client behind the existing status seam`**
Files: new `src-tauri/src/services/remote_flags.rs`; edit `services/service_status.rs`, `services/mod.rs`, `models/service_status.rs` (add internal `FeatureFlag { key, enabled, message, conditions, updated_at }` + `FlagClass`), `utils/http_client.rs` untouched (use `build_client` with UA `MeedyaDL/<version>`).
What: implement the API client (GET features list, 10 s timeout, envelope parse, map namespaced keys → the existing `ServiceStatusConfig` evaluated view so **store/banner/dispatch code is untouched**), credentials via `option_env!("INTAPPS_APP_ID")` / `option_env!("INTAPPS_API_KEY")` (exact same pattern as `APPLE_DEVELOPER_TOKEN` / `ACOUSTID_API_KEY`; absent at dev-build time → client is inert and the old fallback chain rules). `fetch_service_status` becomes: API → (optionally static JSON) → cache → default. Replace the cache write with `atomic_write_json`. Fix the `YouTubeMusic` seed inconsistency.
Tests: unit tests for response mapping, envelope errors, missing-credential inertness; keep all existing `service_status` tests green.

**Commit 2 — `feat(flags): client-side condition evaluation`**
Files: `remote_flags.rs`, `services/gamdl_capabilities.rs` (read-only use).
What: evaluate `conditions` (app version semver range, platform, channel, engine-version range) locally; deterministic rollout bucket persisted in app data (never transmitted). Language-neutral condition schema per Part 5.
Tests: table-driven evaluator tests (boundary semver, unknown condition kinds → **treated as no-match, flag stays enabled** — fail-open for kill-switch class per Part 2 table).

**Commit 3 — `feat(flags): payload signature verification`** *(sequenced with API commit 4-series; ship together or ship this dormant)*
Files: `remote_flags.rs`, `src-tauri/Cargo.toml` (`ed25519-dalek` — permissive licence, passes `deny.toml`; ACKNOWLEDGEMENTS.md + licence checks per #806).
What: verify `X-Payload-Signature` over the exact raw body bytes with an embedded public key; on failure treat as fetch failure (fall to cache). Key absent server-side → header absent → accept-but-log during rollout window; flip to require-signature in a follow-up once the API ships it (record that flip as its own commit).
Tests: known-answer signature vectors shared with the API test suite (same vectors in both repos — cross-repo contract test).

**Commit 4 — `feat(flags): wire polling + surface the banner`**
Files: `src/lib/tauri-commands.ts` (+`checkServiceStatus`), `src/types` (+`ServiceStatusConfig`), rewrite `src/stores/serviceStatusStore.ts` on `createAsyncResourceStore` (its own doc-comment cites this store as the target shape) and **remove `@ts-nocheck`**, `src/components/common/ServiceStatusBanner.tsx` (remove `@ts-nocheck`, fix the `getServiceLabel` import against the real `url-parser.ts` export), `src/components/layout/MainLayout.tsx` (render below UpdateBanner), `App.tsx` (startup check + 6 h interval, same shape as Effect 4).
Also: register the check in `tools/audit-checks/check_ipc_commands.py`'s world by virtue of the wrapper now existing (that checker verifies invoke↔handler pairing — this commit is what makes the dormant pair legal).
Tests: Vitest store tests (fixtures via `src/testing/fixtures.ts`), banner render test for disabled + global-message states.

**Commit 5 — `feat(flags): enforce at download preflight`**
Files: `src-tauri/src/commands/gamdl.rs` (`start_download`), `download_queue.rs` (queue preflight), `src/components/download/DownloadForm.tsx` (fourth pre-download gate, keyed toast per the existing preflight-toast convention).
What: call the already-existing `service_dispatch::is_service_remotely_enabled()` (cached read, no network — its design intent) at enqueue; blocked → activity-log entry via `emit_app_log` + persistent warning toast with the flag's message. **Never cancels active downloads.**
Tests: Rust unit test with a written cache fixture; existing `set_error`/terminal-state guard tests untouched.

**Commit 6 — `feat(flags): engine, stage and premium gates`**
Files: `engine_registry.rs` (`resolve_engine_chain` filters remotely-disabled engines — engine chains already handle arbitrary lengths per the fallback-chain design), `download_queue.rs` enrichment task (per-stage flag check at the same iteration boundaries the `ShutdownSignal` checks use), `apple_music_api.rs` (`resolve_premium_feature_token` gate).
What: skipped stages log "skipped — temporarily disabled" via the stage-label helpers (`set_stage_with_label`). Client sanity floor: the flag fetcher and the updater are hard-excluded from gating (assert in code + test).
Tests: per-gate unit tests; a test asserting the two ungateable capabilities.

**Commit 7 — `chore(flags): settings + docs`**
What: **no new user-facing setting** (an opt-out toggle would defeat the mechanism's purpose; the app already performs unauthenticated update polls to GitHub — document the new poll in README privacy notes and TERMS instead, listing exactly the Part 2 minimum fields). Since no `AppSettings` field is added, **no `settings_version` bump (7→8) is needed** — if review decides a diagnostic toggle *is* wanted, that becomes a serde-defaulted field + v7→v8 migration per convention. Update README.md, help/faq.md ("why is a service unavailable?"), DEV_NOTES.md, CLAUDE.md.
`Release-Note: none` (docs) / plain-English notes on the feature commits as above.

CI/verification: `cargo test`, `npm run test`, `npm run type-check`, the two `tools/audit-checks` scripts, and a manual end-to-end against a staging flag ("disable Spotify" → banner appears, enqueue blocked, re-enable → `preflight-cleared` toast dismissal path).

---

## Part 4 — Implementation plan, intAppsAPI

### Threat model first

| Adversary | Capability | What defends | Honest limit |
|---|---|---|---|
| (a) Binary reader | Extracts embedded App-ID, API key, HMAC secret, signing **public** key from any shipped client | Nothing prevents extraction. Scoped **read-only** keys for clients (no write scope ⇒ fail-closed scope check blocks mutations even with the key); per-app revocation + rotation; rate limiting | The per-app key is **shared across all installs and must be treated as semi-public**. It is an *attribution and abuse-filtering* signal, not a secret boundary. Never gate anything sensitive on possession of it. |
| (b) Network path attacker | MITM, DNS spoof, captive portal | TLS (floor) + **Ed25519 payload signature** (new) — forged flag payloads rejected even through interception proxies | Signature key on the same host as the web tier (shared hosting) ⇒ web-tier compromise defeats it. Defence-in-depth, not a boundary. |
| (c) User defeating the kill switch on their own machine | hosts file, firewall, patched binary | Sticky last-known-good cache raises effort above "block one domain" | **Fundamentally advisory.** Client-side enforcement on user-owned hardware cannot guarantee compliance. If a legal obligation requires a *guarantee*, this mechanism cannot provide one and counsel should be told so in those words. |
| (d) Compromised API / admin account | Push a malicious flag to the whole installed base | Least-privilege admin roles (admin/viewer exist), **audit wiring (currently absent — must add)**, mass-disable email alert, staged rollout, instant revert, client sanity floor (can't disable fetcher/updater), fail-open client (worst malicious push = features *off*, not code execution — flags carry no executable content, keep it that way: no URLs the client auto-fetches, no format strings) | Blast radius of a bad push is bounded to "features disabled until revert + ≤6 h propagation". This bound is the design's most important property — protect it (never add remote-code-shaped metadata). |
| (e) Accidental bad flag push | Same reach as (d) | Same containment + admin-UI confirm friction on `kill_switch`-class flags + audit trail for post-mortem | — |

Verification hooks are listed per commit below; every control gets a test or a named manual check.

### Setup (before commits)

- **Create `alpha` branch** from `main` (user directive: work is alpha-based per repo; the branch does not exist). Mind the branch-protection note: required checks `Backend (ubuntu-latest)` / `Frontend (ubuntu-latest)` exist only for PRs into `main`; decide whether to mirror protection onto `alpha`.
- **Decision needed:** integrate the unmerged `claude/feature-gating-readiness-yQisQ` branch (rollout targeting, migration 015) first, as the repo's own audit recommends. Plan below assumes **yes**; if no, renumber the new migration to 016 and re-implement targeting fresh (duplicated effort — not recommended).

**Commit A — Integrate branch B** (if accepted): migration 015 `rollout_strategy`, feature webhook/audit wiring it reportedly carries; apply the repo-memory adaptation checklist (batch-scope decision, `features.js` `rolloutSummary()` JSON-string guard, openapi version). *Contents unverifiable in this clone — verify at integration time.*

**Commit B — Migration `016_add_flag_classes_and_targeting.sql` (+ `_rollback`)**
```sql
ALTER TABLE `tblAppFeatures`
  ADD COLUMN `flag_class` ENUM('kill_switch','operational','experiment') NOT NULL DEFAULT 'operational'
    COMMENT 'Drives client fail policy and admin-UI confirm friction' AFTER `is_enabled`,
  ADD COLUMN `client_message` VARCHAR(500) NULL DEFAULT NULL
    COMMENT 'User-facing text shown by the client when the flag disables something' AFTER `flag_class`,
  ADD COLUMN `conditions_json` JSON NULL DEFAULT NULL
    COMMENT 'Client-evaluated targeting: min/max app version, platforms, channels, engine version ranges' AFTER `metadata_json`;
```
(If branch B's `rollout_strategy` covers targeting adequately, drop `conditions_json` and extend that instead — decide at integration.) Rollback drops the three columns. Model (`Feature.php`): extend SELECT/INSERT/UPDATE field lists (prepared statements, `sss` type extensions). `formatFeature()` gains `flag_class`, `client_message`, `conditions` (decoded), `updated_at`. **Relax `feature_key` charset**: new `InputSanitizer::flagKey()` allowing `^[a-z0-9][a-z0-9._-]*$` (lowercase, dot-namespaced, ≤100) used by feature routes only — with unit tests including rejection cases; keep `slug()` untouched for everything else.
Tests: model round-trip unit tests; migration applied+rolled back in CI (MySQL service container — CI shape to confirm in `ci.yml`).

**Commit C — Response payload signing**
New `Helpers/PayloadSigner.php`: `sign(string $rawBody): ?string` using `sodium_crypto_sign_detached` (libsodium is bundled with PHP ≥7.2 — no Composer dep, consistent with the no-runtime-deps rule); key from env `FLAG_SIGNING_SECRET_KEY` (base64), validated at boot by `ConfigValidator` (warn-absent in dev, require in prod — mirror the HMAC master-key ≥32-char precedent). `Response::successSigned()` variant (or an opt-in flag on `success()`) that emits `X-Payload-Signature: <hex>` + `X-Payload-Key-Id`. Wire into the three feature endpoints. **Signature covers the exact bytes sent** — no canonicalisation step, so any-language clients verify raw bytes (Part 5 requirement).
Also: conditional GET on feature endpoints — compute a strong ETag (hash of payload), honour `If-None-Match` → 304 with signature header repeated; carve the `no-store` header out for these routes only (keep for everything else).
Tests: known-answer vectors (shared with MeedyaDL commit 3), tamper test, 304 path test. Key generation + storage documented in `DEPLOYMENT.md`; **never commit key material** (gitleaks in `pr-security.yml` is the CI backstop).

**Commit D — Audit + alerting on flag mutations**
Wire the existing-but-unused `AuditLogger` into `Admin/FeatureController::{create,update,delete}` (actor, app, key, before→after diff). Dispatch a webhook event (`feature.updated`) via the existing queue. Mass-disable alert: if >N `kill_switch` flags across an app flip to disabled within a window, send an ops email via `MailClient` (threshold in config).
Tests: audit-row assertions per mutation (first Integration-tier tests — establish `web/tests/Integration/` with a transactional DB fixture, filling the currently-empty directory); alert threshold unit test.

**Commit E — Admin UI (`features.js` + helpers/css)**
Flag-class selector; `client_message` field; conditions editor (structured: version range, platform multi-select, channel multi-select — not raw JSON); **confirm modal with typed app-slug for disabling a `kill_switch` flag** (blast-radius friction, threat (e)); per-flag change-history drawer reading the audit log; badge colouring respecting the CVD-safe theme (shape+colour per the Wong-palette convention in DEV_NOTES). All output through the existing `escapeHtml` discipline.
Tests: manual checklist (the admin SPA has no JS test rig — note as accepted gap; add to issue #101's a11y pass while touching these screens).

**Commit F — Docs & contract**
`web/docs/openapi.json`: new response fields, signature/ETag headers, key-charset change; README feature matrix; CHANGELOG (Keep-a-Changelog); onboarding runbook (Part 5 steps) in `DEV_NOTES.md`. Version bump 0.3.0 → 0.4.0 (SemVer minor: additive).

**Cross-cutting verification:** every new query is a prepared statement (grep gate in `php-static-analysis.yml` conventions); all new inputs through `InputSanitizer`; admin routes keep `AdminAuth`+`AdminRole` middleware; app routes keep the 3-factor stack; run the full unit+integration suite in CI; manual pen-check that a read-scoped key still 403s on admin paths and that a wrong-UA request 403s generically and lands in the auth-fail rate limiter.

### Honest assessment of the User-Agent factor (per direction)

Verified real and enforced (Part 1b). It is **trivially spoofable** — the UA string ships inside every client binary and is visible to any on-path observer. What it buys: (1) a cheap filter against accidental/casual traffic and scanner noise; (2) request attribution for logs and analytics; (3) one more thing a lazy abuser must copy. What it does not buy: any security boundary whatsoever. The actual boundary layering is: TLS → per-app scoped API key (semi-public, revocable, rate-limited — abuse control, not secrecy) → HMAC app secret for mutations (the only genuinely secret client credential; for flag-*reading* clients like MeedyaDL, **do not embed the HMAC secret at all** — clients that never mutate should never carry it) → server-side payload signing for integrity. Design docs and admin UI copy should describe UA-checking as "attribution", never "authentication".

---

## Part 5 — Suite-wide generalisation

### Verified facts about the sibling apps

- **MeedyaConverter** (`/workspace/meedyaconverter`, `main`): **Swift 6.3**, SwiftPM (`Sources/{MeedyaConverter,ConverterEngine,meedya-convert}`), proprietary licence, v0.1.0-rc.3. Update mechanism: `GitHubReleaseChecker.swift` — `URLSession` + `JSONDecoder`, 1 h cache, GA-only, deliberately unauthenticated; a planned v0.2.0 path references a **Sparkle EdDSA keypair + an `update.mwbm.io` Cloudflare Worker (issue #416)** — in-house precedent for signed remote feeds. **No remote-config/kill-switch mechanism found** (grep across `Sources/`). Local clone has `main` only (remote `alpha`/`beta` exist per session setup but were not fetched — sibling analysis is `main`-based).
- **MeedyaManager** (`/workspace/meedyamanager`, `main`): **Rust workspace** (crates `mm-core`, `mm-update`, `mm-cli`, `mm-gtk`, `mm-server`, …), GPL-2.0+ badge in README, native UIs (SwiftUI / WinUI 3 in C# / GTK4). Update mechanism: `mm-update` crate, `reqwest` against GitHub Releases, semver-aware, prerelease-pref aware. **Already consumes `meedya-core` from `MWBMPartners/MeedyaSuite-core`** (workspace dep pinned to rev `222ca7590493`, `features=["full"]`). No remote-config mechanism found.
- **Neither sibling (nor MeedyaDL) references `api.mwbmpartners.ltd` or intAppsAPI anywhere in source** (grep-verified; MeedyaConverter has only a marketing-site URL in `AppInfo.swift`).
- **`MeedyaSuite` and `Skriptey` orgs, and the contents of `MWBMPartners/MeedyaSuite-core` itself: NOT verified** — not cloned, no GitHub CLI in this session. Everything below concerning them is plan-only and must not be assumed to resemble the three verified repos.

### The contract (design once, language-neutral)

Wire format rules — **no Rust-shaped assumptions**: plain JSON objects; enums as lowercase strings (`"kill_switch"`, never serde-tagged unions); timestamps as ISO 8601 UTC strings; optional = absent-or-null, clients treat both identically; unknown fields ignored (forward compat); unknown enum values / condition kinds treated as no-match-fail-open. **Signature verifies the raw transmitted bytes** — no cross-language canonical-JSON problem exists by construction.

Flag response item (server → client):
```json
{
  "feature_key": "service.apple-music",
  "label": "Apple Music downloads",
  "enabled": true,
  "flag_class": "kill_switch",
  "client_message": null,
  "conditions": { "min_app_version": "1.11.0", "platforms": ["macos","windows"], "channels": ["stable","rc"] },
  "metadata": {},
  "updated_at": "2026-07-27T00:00:00Z"
}
```

**Canonical namespace** (dot-separated, lowercase kebab segments — requires the Part 4 `flagKey()` sanitiser change): `service.<id>`, `engine.<id>`, `engine.<id>.version` (conditions carry the range), `enrichment.<stage>`, `premium.<feature>`, `app.announcement`-class things go to notifications instead. Per-app scoping is implicit (`app_slug` in the URL); keys are therefore short and app-local — the same key string may exist for several apps with independent state.

**Per-app onboarding runbook:** (1) admin `POST /v1/admin/apps` with name, slug, `user_agent_prefix` = `"<AppName>/"` — response returns the API key and HMAC secret exactly once; (2) create a **read-only scoped key** for the shipped client (`permissions: ["read"]`) — the app-level key and HMAC secret stay in ops vaults, never in binaries of read-only clients; (3) store App-ID + scoped key as CI secrets, inject at build time (MeedyaDL: `option_env!`; MeedyaConverter: xcconfig/`Package.swift` build settings; MeedyaManager: `option_env!`); (4) seed the app's flag set + smoke-test one flag end-to-end; (5) rotation drill: create new scoped key → ship a release → revoke old key after adoption window (the admin list/create/revoke endpoints already exist).

**Reference client behavioural spec** (what every implementation must do): fetch on startup + every 6 h with jitter; send only App-ID + key + versioned UA; verify signature when present (hard-require once the fleet ships verification); honour ETag; atomic-write cache; sticky last-known-good for `kill_switch`; compiled-in defaults for unknown flags; client-side condition evaluation; never transmit evaluation inputs; never gate own fetcher/updater; enforce at operation start, not mid-operation; log flag decisions without credentials.

**Shared crate vs. per-app clients — recommendation:** *tightly-specified wire contract + small per-app clients now; extract later.* Reasons: (1) only two of three apps are Rust, so a crate can never be "the" implementation — the Swift client must be written against the contract regardless, which forces the contract to be the real artefact; (2) the client is small (~300–500 lines each: HTTP, verify, cache, evaluate) and each app's cache/settings/logging idioms differ (Tauri app-dir + `atomic_write_json` vs `mm-core` config vs `URLSession` + Application Support); (3) MeedyaSuite-core's contents/conventions are unverified this session, and MeedyaDL's own precedent shows shared-crate extraction works best *after* two consumers exist with converged code (`meedya-fingerprint` was extracted from working MeedyaDL code, per its CLAUDE.md). When MeedyaManager actually implements, extract `meedya-remote-config` into MeedyaSuite-core from MeedyaDL's client — **licence it MIT or MIT/Apache-2.0** so it is consumable by MIT MeedyaDL and GPL-2.0+ MeedyaManager alike (a GPL'd shared crate would be unusable by MeedyaDL; verify MeedyaSuite-core's licensing conventions before extraction — unverified here). The contract document + shared signature test vectors live in intAppsAPI (`web/docs/`) as the single source of truth, versioned with the API.

---

## Part 6 — Issue plan

**intAppsAPI** (all implementable now; sequential):
1. *"Create `alpha` working branch + decide branch protection"* — setup; labels: `infrastructure`. Blocks all below if alpha-based flow is required.
2. *"Integrate feature-gating-readiness branch (rollout targeting, migration 015)"* — body: repo-audit recommendation, adaptation checklist, note that no issue currently tracks the rollout feature (repo memory flags this). Labels: `enhancement`. Blocks 3.
3. *"Flag classes, client messages, targeting conditions (migration 016)"* — schema+model+sanitiser; depends on 2.
4. *"Ed25519 response signing + conditional GET on feature endpoints"* — depends on 3; links MeedyaDL issue 3 below as consumer.
5. *"Wire AuditLogger into admin flag mutations + mass-disable alerting"* — body cites the verified zero-call-site finding. Independent; do early.
6. *"Admin UI: flag class/conditions/message editors, kill-switch confirm, history drawer"* — depends on 3, 5; cross-link a11y issue #101.
7. *"Publish remote-config wire contract + shared signature test vectors in web/docs"* — depends on 3, 4.
8. *"Ops: confirm production deployment of api.mwbmpartners.ltd (incl. .htaccess DirectoryMatch fix, PHP 8.4)"* — **blocking for all client cutovers**; labels: `deployment`, `blocked?`.

**MeedyaDL** (per repo convention: create issue, close with comment, add to project 6, link parents):
1. *"Cut service-status transport over to intAppsAPI (kill-switch goes live)"* — umbrella; commits 1–7 of Part 3; depends on intAppsAPI 3 & 8. Labels: `enhancement`, `security`.
2. *"Activate dormant serviceStatusStore/ServiceStatusBanner (remove @ts-nocheck, wire polling)"* — child; implementable **now** even against the static-JSON transport (the seam makes it transport-independent) — the only part not blocked on the API.
3. *"Verify signed flag payloads"* — child; blocked on intAppsAPI 4.
4. *"Engine/stage/premium-feature remote gates"* — child; depends on 1.
5. *"Decide + publish service-status.json on main (bridge) or document direct cutover"* — decision issue; see Part 7.

**MeedyaConverter / MeedyaManager** (plan-only until their maintainers schedule): one issue each — *"Implement suite remote-config client per intAppsAPI contract"* — blocked on intAppsAPI 7 & 8; MeedyaManager's issue also references the existing `feature/MeedyaManager_MeedyaSuite-core_integration` branch and the later `meedya-remote-config` extraction. **MeedyaSuite-core:** one plan-only issue for the eventual crate extraction — blocked on a second Rust consumer existing; repo conventions unverified.

---

## Part 7 — Risks and open questions (human decisions needed)

1. **Is the API deployed, and to what availability standard?** Unverifiable this session. A kill switch on a host that 500s (the known `.htaccess` `<DirectoryMatch>` issue would 500 *every* request on DreamHost) is worse than the interim file on GitHub's CDN. Decision: confirm deployment + fix the blockers **before** any client cutover; consider whether GitHub-file-as-fallback should stay in the chain precisely because its availability exceeds shared hosting's.
2. **Fail-open is a policy statement about the kill switch.** Trade-off: fail-closed would make the switch robust against domain-blocking but would brick offline users and contradict MeedyaDL's offline-capable design. Recommended: fail-open + sticky cache, and an explicit internal note that **client-side enforcement is advisory** — if a legal obligation ever requires a guarantee, this mechanism cannot supply one and that must be said to whoever relies on it.
3. **No user opt-out of flag polling** is recommended (an opt-out defeats the mechanism; the poll sends less than the existing GitHub update check). But it is a privacy-posture decision the maintainer should ratify explicitly, and the poll must be documented in README/TERMS. If ratified the other way, a settings field + `settings_version` 7→8 migration is required.
4. **Signing-key custody.** On shared hosting the Ed25519 secret key lives beside the web runtime, so signing does not survive full server compromise — accept as defence-in-depth, or move signing (or the whole flag read path) to a Cloudflare Worker where the key is a bound secret (there is in-house precedent: MeedyaConverter's planned `update.mwbm.io` Worker; Cloudflare accounts exist in this session's tooling). Recommendation: accept for v1, note the Worker as the upgrade path.
5. **Branch B (migration 015) integration** in intAppsAPI: recommended yes (repo's own audit), but it is three-months idle, unverifiable in this clone, and needs an owner. If declined, targeting is re-implemented fresh under migration 016 and the 015 slot must be treated as burned.
6. **Bridge file or direct cutover?** Since no shipped MeedyaDL build ever polled the interim URL, the bridge (publishing `service-status.json` on `main`, generated from the API) protects nobody today. Recommendation: skip it — but this assumes no interim-mechanism-activated release ships before the cutover; whoever controls the release train should confirm.
7. **Mid-download flag flips** are deliberately not enforced (gate at enqueue only). If legal circumstances ever require stopping in-flight downloads, that is a separate, user-hostile feature that should be its own decision.
8. **Unverified items, consolidated:** production reachability of `api.mwbmpartners.ltd`; contents of the two unmerged intAppsAPI branches; remote `alpha`/`beta` branches of the sibling apps (local clones are `main`-only); contents/licensing conventions of `MWBMPartners/MeedyaSuite-core`; everything in the `MeedyaSuite` and `Skriptey` orgs; whether intAppsAPI CI provisions a MySQL service for integration tests (shapes Part 4 commit D's test design).
