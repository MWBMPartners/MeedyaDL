---
name: project-build-time-secrets
description: Build-time values must be wired into release.yml or the feature ships completely inert and silent — three shipped features were in that state
metadata:
  type: project
---

Several MeedyaDL features are switched on by a value supplied when the app is built, not by a
setting a user can see. Rust reads them with `option_env!("NAME")`; the frontend reads them as
`import.meta.env.VITE_NAME`. Every one treats a missing value as "not configured" and then,
deliberately, says nothing at all.

On 2026-09-08 a sweep found **three finished, reviewed, shipped features that had never once worked
in a build anyone could install**, purely because the value was never passed through `release.yml`:

- **Crash reporting** (`SENTRY_DSN` / `VITE_SENTRY_DSN`) — #1161. Had been "fixed" in #231 back in
  March by making the code read an environment variable. Nobody set the variable, so the original
  complaint ("users who opt-in believe they're contributing crash data but nothing is sent") stayed
  true for five months while the issue sat closed.
- **The remote pause switch** (`INTAPPS_BASE_URL` / `_APP_ID` / `_API_KEY`) — #1163. The mechanism
  for stopping a service across every installed copy without shipping an update. Inert in every
  build, so the emergency lever would have been found disconnected at the moment it was needed.
- **Developer access** (`DEV_ACCESS_HASH`) — #1162. Worse than inert: the check fell back to
  accepting the hash of an empty string, so the hidden gate opened by pressing the button without
  typing anything. That unlocked Spotify downloading, the daily-counter reset, and the unstable
  update channels.

**Why:** absence is indistinguishable from success. Each feature is individually correct to go quiet
— that is right for a fork or a local build — but nothing anywhere reports the combination, so a
feature can be complete, tested, merged and dead with no signal at all. This is the same shape as
every other silent failure found that week.

**How to apply:** adding a new `option_env!` or `import.meta.env` read means adding it to **all
three** `env:` blocks in `release.yml` in the same change. An uncreated secret resolves to an empty
string, so wiring it early is always safe and the feature switches itself on the moment the secret
exists — there is never a second workflow change needed later.
`tools/audit-checks/check_build_secrets.py` enforces this on every PR. It checks the *wiring*, not
whether a secret has a value on GitHub — that needs admin rights CI does not have and should not
have.

Related: [[project-crash-reporting-backend]], [[project-alpha-main-drift]]
