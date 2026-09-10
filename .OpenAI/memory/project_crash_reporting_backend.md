---
name: project-crash-reporting-backend
description: Crash reports go to GlitchTip, not Sentry — but the code is identical, so never add a second SDK
metadata:
  type: project
---

Decided 2026-09-08 (#1161). **GlitchTip accept Sentry's own client libraries** — "Our app is
compatible with Sentry client SDKs" — so the `sentry` crate and `@sentry/browser` already in
MeedyaDL send to either service completely unchanged.

**Which backend receives reports is decided entirely by the DSN.** Switching between GlitchTip,
Sentry, or a self-hosted instance later is one GitHub Secret, not a code change.

**Never add a second reporting library believing two are needed.** That is the mistake this note
exists to prevent.

**Why GlitchTip:** Sentry's free tier allows **exactly one user to sign in**, which rules it out for
something meant to cover several MWBM and MeedyaSuite applications. GlitchTip allow unlimited team
members at £0 (1,000 reports/month), then $15/month for 100,000 against Sentry's $26/month for
50,000. Going over the free allowance throttles rather than bills — they drop 10% at the limit,
rising to a full block at twice the quota — so there is no surprise-invoice risk and the free tier
is safe to switch on unwatched.

**Neither can run on DreamHost shared hosting.** GlitchTip needs PostgreSQL 14+ (shared gives
MySQL), a continuously running Django process plus background workers, and a Python environment;
Sentry self-hosted needs Docker with 16 GB RAM. A small VPS (~€4/month) would work if owning the
data at `sentry.mwbm.cloud` ever becomes a requirement in its own right.

**How to apply:** before doing anything else here, note that the DSN was never wired into the
release build at all, so nothing has ever been sent from any version — see
[[project-build-time-secrets]]. #998 (the app's own security rules do not permit reaching a
reporting service) must land alongside, or the frontend half stays silently broken behind the fix.

Full comparison and costs: `.github/audits/crash-reporting-hosting-options-2026-09-08.md`
