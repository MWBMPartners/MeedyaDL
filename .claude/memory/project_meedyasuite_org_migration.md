---
name: project-meedyasuite-org-migration
description: Planned consolidation of MWBMPartners/* repos under MeedyaSuite org — secrets list, transfer order, runbook prerequisites. NOT YET EXECUTED.
metadata:
  type: project
---

# MeedyaSuite org consolidation — pending migration

**Status (2026-05-22):** Decided to do this, but **NOT YET**. Deferred until after v1.10.0-alpha.1 (GAMDL 3.6) ships and stabilises through alpha/beta/RC into v1.10.0-stable. Two concrete deliverables landed today: (a) `tauri.conf.json` updater endpoints array now includes both `MWBMPartners/MeedyaDL` and `MeedyaSuite/MeedyaDL` so existing installs survive the transfer regardless of redirect behaviour; (b) GitHub Issue filed on `MeedyaSuite/MeedyaDL-Tools` for the secrets migration with full step-by-step runbook.

**Why:** Group all related apps/tools in one org. Sibling repos already on MeedyaSuite (`MeedyaDL-Tools` → to be renamed `MeedyaSuite-Tools`; `MeedyaPlayer` private). Local working tree already lays them out under `MeedyaSuite/` parent.

**How to apply:** When the user revisits the migration, treat the GitHub Issue (on `MeedyaSuite/MeedyaDL-Tools`) as the canonical runbook. The list of secrets and the transfer order are pinned there — don't reconstruct them from memory.

## Repos to move (current → MeedyaSuite)

| Current location | Destination | Action |
|---|---|---|
| `MWBMPartners/MeedyaDL` | `MeedyaSuite/MeedyaDL` | Transfer |
| `MeedyaSuite/MeedyaDL-Tools` | `MeedyaSuite/MeedyaSuite-Tools` | **Rename only** (already on org) |
| `MWBMPartners/MeedyaPlayer` (or wherever) | `MeedyaSuite/MeedyaPlayer` | Transfer (private repo, already exists at destination per `gh repo list` 2026-05-22) |
| `MWBMPartners/MeedyaConverter` | `MeedyaSuite/MeedyaConverter` | Transfer |
| `MWBMPartners/MeedyaSuite-core` | `MeedyaSuite/MeedyaSuite-core` | Transfer (check current location — may already be on MeedyaSuite) |
| `MWBMPartners/MeedyaManager` | `MeedyaSuite/MeedyaManager` | Transfer |
| `MWBMPartners/MeedyaDB` | `MeedyaSuite/MeedyaDB` | Transfer |

**Verify current locations with `gh repo list <org>` before the migration day** — the user said MeedyaDL is on MWBMPartners but didn't confirm the others. Don't assume.

## Secrets in scope (MeedyaDL workflow inventory, 2026-05-22)

Grep'd from `.github/workflows/`:

| Secret | Used by | Scope | Sensitivity |
|---|---|---|---|
| `RELEASE_PAT` | release-please-action, channel-release workflows (tag pushes that need to re-trigger downstream workflows) | Classic PAT, owner-scoped | **High** — has push access to all repos the owner can write |
| `TAURI_SIGNING_PRIVATE_KEY` | release.yml, channel-release workflows | Base64-encoded `.key` file from `cargo tauri signer generate` | **Critical** — controls updater signing; loss = lose ability to ship signed updates |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | release.yml, channel-release workflows | Password for the `.key` file | **Critical** — paired with above |
| `APPLE_CERTIFICATE` | release.yml (macOS jobs) | Base64 of Developer ID Application `.p12` | **High** — codesigning identity |
| `APPLE_CERTIFICATE_PASSWORD` | release.yml (macOS jobs) | `.p12` password | **High** |
| `APPLE_SIGNING_IDENTITY` | release.yml (macOS jobs) | String, e.g. `"Developer ID Application: MWBM Partners Ltd (XXXXXXXXXX)"` | Medium |
| `APPLE_ID` | release.yml (notarization) | Apple ID email | Medium |
| `APPLE_PASSWORD` | release.yml (notarization) | App-specific password (NOT Apple ID password) | High |
| `APPLE_TEAM_ID` | release.yml (notarization) | 10-char Apple Team ID | Low |
| `ACOUSTID_API_KEY` | release.yml (passed as env to cargo build for `option_env!`) | AcoustID free-tier API key | Low |

**Not yet wired in CI but referenced in code via `option_env!`:**
- `MUSICKIT_DEVELOPER_TOKEN` (compile-time embedded Apple Music JWT for premium features)
- `DEV_ACCESS_HASH` (SHA-256 hash for Konami dev-access sentinel)
- `SENTRY_DSN` (optional)

These three will need wiring at the same time as the migration if they get integrated into CI.

**`GITHUB_TOKEN` is auto-provided per-workflow** — not a stored secret. Don't migrate.

## Recommended secret placement on MeedyaSuite

Three tiers:

- **Org-level, all repos** (shared across the whole suite): `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` — every app on MeedyaSuite that ships a macOS build needs the same Apple credentials. Save once, share everywhere.
- **Org-level, selected repos**: `RELEASE_PAT`, `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — only repos that actually publish releases via Tauri. Most likely MeedyaDL, MeedyaPlayer, MeedyaConverter; not the libraries (`MeedyaSuite-core`, `MeedyaDB`).
- **Repo-level, MeedyaDL only**: `ACOUSTID_API_KEY` — specific to MeedyaDL's AcoustID enrichment. Other apps don't need it. (When MUSICKIT_DEVELOPER_TOKEN / DEV_ACCESS_HASH get wired, same logic applies.)

## Why this isn't done yet

1. **Transfer is irreversible mid-EPIC.** GAMDL 3.6 alpha is in flight (PR #855 merged, v1.9.4-alpha.10 building). Transferring during release pipeline activity risks losing in-flight artifacts.
2. **Tauri updater URL is baked into every installed copy.** Existing v1.x users check `https://github.com/MWBMPartners/MeedyaDL/releases/.../latest.json` on launch. GitHub's repo-transfer redirect works, but is brittle if anyone ever creates a new repo at the old name. **Mitigated** as of 2026-05-22 by adding the MeedyaSuite endpoint as a second entry in `tauri.conf.json`'s `endpoints` array — Tauri tries each in order, so installs that ship from 2026-05-22 onwards work whether the repo lives at MWBMPartners or MeedyaSuite.
3. **Cross-repo references audit not yet run.** README/SECURITY/CLAUDE.md/help/.github/Cargo.toml/package.json/crash-report URL builder/etc. all reference `MWBMPartners/MeedyaDL`. ~20–40 sites needing find-replace. To be batched into a single prep PR pre-transfer.

## GitHub does NOT support nested orgs

(Documented here because this came up during the planning conversation.)

GitHub's hierarchy is strictly two-level: `org → repo`. There is no parent/child relationship between orgs. `MWBMPartners` and `MeedyaSuite` are siblings. Child orgs cannot inherit secrets from a parent org because no such relationship exists.

Approximations:

- **Org-level secrets with selective repo access** (free) — set once on the org, allow specific repos to read.
- **GitHub Enterprise Cloud** (paid) — enterprise-wide secrets/policies across multiple orgs. Overkill at current scale.
- **External secret store + OIDC** (HashiCorp Vault, AWS Secrets Manager, 1Password Connect) — orgs read from a single source via short-lived tokens.

**For our migration: option 1.** Duplicate secrets to MeedyaSuite org at migration time. One-time chore.

## Related

- [[project_github_orgs]] — existing note about MWBMPartners + MeedyaDL orgs in cargo-deny allowlist (predates this consolidation plan; will need updating)
- [[project_brand_identity]] — MeedyaDL = product, MeedyaSuite = vendor; consolidation completes the vendor-namespace alignment
- [[project_macos_updater_bug]] — earlier updater filename mismatch (unrelated, but updater-related)
