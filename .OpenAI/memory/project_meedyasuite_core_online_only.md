---
name: project-meedyasuite-core-online-only
description: For any MeedyaSuite-core integration work, ALWAYS check the upstream repo online (github.com/MWBMPartners/MeedyaSuite-core) — never trust a local cargo-registry checkout, vendored copy, or stale clone.
metadata:
  type: feedback
---

**Standing rule (set 2026-05-18):** Whenever investigating, planning, or
implementing any integration with the MeedyaSuite-core library (the
`meedya-codecs`, `meedya-fingerprint`, `meedya-metadata`, `meedya-db`,
`meedya-core` crates), check the upstream repository **online**:

> Repo: https://github.com/MWBMPartners/MeedyaSuite-core

Do NOT use any of the following as the source of truth:
- `~/.cargo/git/checkouts/meedyasuite-core-…/` — frozen at the
  Cargo.toml-pinned commit, may lag main by weeks.
- Vendored copies under `vendor/` or `third_party/` (if any).
- Local clones in adjacent directories (`../MeedyaSuite-core/`,
  `~/Projects/MeedyaSuite-core/`).
- The cached `cargo doc` output under `target/doc/meedya_*/`.

**Why:** during the v1.7 bumper-bundle session I checked the local
cargo checkout for #352/#353/#596 work. That checkout was pinned at
the `claude/interesting-mirzakhani` branch and missed any upstream
changes since. For #596 (LyricsFile) the local checkout showed no
`meedya-lyrics` crate — but the *online* repo could have shipped it
between Cargo.toml's pin date and the session. The integration plan I
posted assumed "blocked on upstream" when it might actually have been
"unpin Cargo.toml's branch reference and bump the commit hash".

**How to apply:**
1. Use `gh repo view MWBMPartners/MeedyaSuite-core --json defaultBranchRef` and `gh api repos/MWBMPartners/MeedyaSuite-core/contents/<path>` to inspect the live tree.
2. For Rust APIs, fetch `crates/<crate-name>/src/lib.rs` (and `Cargo.toml`) via the GitHub Contents API rather than relying on the local cargo metadata.
3. When proposing a "bump the dep" change in MeedyaDL, compare the live upstream commit SHA (`gh api repos/MWBMPartners/MeedyaSuite-core/commits/main --jq '.sha'`) against what's in MeedyaDL's `Cargo.toml` to identify drift.
4. If the upstream `branch` reference in `Cargo.toml` is a feature branch (e.g. `claude/interesting-mirzakhani`), check whether that branch still exists and whether the work has been merged to `main` — the right action is usually to switch to `branch = "main"` and bump the hash.

Related: [[project-v17-bumper-bundle]] — the session where this rule was set.
Related: [[project-multi-service-groundwork]] — earlier MeedyaSuite-core context.
