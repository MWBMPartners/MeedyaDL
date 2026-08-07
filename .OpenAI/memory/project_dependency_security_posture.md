---
name: Dependency + secret-scanning security posture (dev-only advisories; updater pubkey false-positive)
description: The npm advisories that fire on MeedyaDL are dev/CI-only (shipped HTTP is Rust reqwest); the tauri.conf.json secret-scan hit is the updater PUBLIC key and must never be "fixed". How to triage dep alerts + the overrides ratchet.
type: project
---
Durable facts for triaging MeedyaDL's GitHub security alerts (recorded 2026-08-05, dependency-consolidation session).

## npm dependency advisories are dev/CI-only — NOT user-facing
MeedyaDL is a Tauri app: the **shipped** app's HTTP is Rust `reqwest` (`src-tauri/src/utils/http_client.rs`), and the frontend is Vite-bundled WebView code where Node HTTP libs cannot ship. So Node/npm advisories (undici, ip-address, fast-uri, brace-expansion, etc.) live **only** in dev/test/CI tooling and have **zero end-user runtime exposure**. Known transitive chains:
- `undici` ← `jsdom` (Vitest DOM test env). `"dev": true`.
- `ip-address` ← `socks` ← `socks-proxy-agent` ← `pac-proxy-agent`/`proxy-agent` ← `@puppeteer/browsers` ← `puppeteer` (icon-generation scripts). `"dev": true`.
- `fast-uri` ← `ajv` (schema tooling). `brace-expansion` ← `minimatch` (eslint/glob).
Triage rule: still fix them (supply-chain hygiene for contributor/CI machines), but **no release note** and no app-side remediation — say so plainly rather than implying a user-facing patch. Verify dev-scope with `node -e` on `package-lock.json` (`"dev": true` + single copy) and `grep` the target under `src/` (expect zero hits).

## The `overrides` ratchet (package.json)
`overrides` pins security floors on transitive deps so they survive lockfile regeneration / Dependabot rebases. Current members: `basic-ftp`, `fast-uri` (`^3.1.5`), `ip-address` (`^10.4.0`), `undici` (`^7.29.0`). When a fixed version is **within** the existing declared range (e.g. brace-expansion 5.0.9 within `^5.x`), a plain `npm update <pkg>` is durable — npm always resolves latest-satisfying — so an override is unnecessary. Add/bump an override only to move the FLOOR (fixed version at/above the pin) or when a single-copy transitive dep wants an explicit ratchet. **Do NOT add a global override for a package with multiple major versions in the tree** (e.g. `brace-expansion` also has 1.x/2.x consumers) — a global override forces every copy to one version and breaks the other-major consumers. CI installs are `npm ci` only (no `npm install`/lock regeneration in workflows), so the committed lock is authoritative.

## Secret-scanning: `tauri.conf.json` "Password" is the updater PUBLIC key — permanent false positive
GitHub secret-scanning periodically flags `src-tauri/tauri.conf.json` `plugins.updater.pubkey` as a Generic "Password". It base64-decodes to `untrusted comment: minisign public key: FE03A1F781F9D761…` — a minisign **public** key the Tauri updater needs embedded to verify update signatures (documented SECURITY.md). It is **safe to ship and must never be removed/rotated as a "leak"**. No private-key material is committed anywhere (no tracked `.p8`/`.pem`/`.key`/`.env`; every `-----BEGIN` in-tree is docs/placeholder/`format!`-assembled test fixture; the real signing key lives only in the `TAURI_SIGNING_PRIVATE_KEY` Actions secret). **Disposition: dismiss the alert(s) as false-positive in the GitHub UI (maintainer action — no MCP/API tool dismisses secret-scanning alerts). Do NOT add `tauri.conf.json` to `.github/secret_scanning.yml` `paths-ignore`** — that would blind scanning on the config file most likely to gain a real credential later. Tracked historically by closed #1032.

## Forward-porting security fixes to channels
Dependabot **security** PRs always open against the default branch (`main`) and ignore `dependabot.yml`'s `target-branch: alpha` (which routes *version* updates only). `.github/workflows/forward-port-security.yml` (added 2026-08-04) cherry-picks a merged Dependabot fix onto `alpha`/`beta`/`release-candidate` and opens a PR each (or a `[forward-port]` issue on conflict) — but it only fires on FUTURE merges. To bring an already-open Dependabot fix onto an in-flight alpha-bound branch, cherry-pick it directly (as done for undici #1079 / ip-address #1078 in the 2026-08-05 session).
