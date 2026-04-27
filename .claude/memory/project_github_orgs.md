---
name: GitHub organisation structure
description: Two GitHub orgs — MWBMPartners (parent company) and MeedyaDL (project-specific repos)
type: project
originSessionId: 3077ffe9-2509-4112-9e18-ef53b04ad9ea
---
Two GitHub organisations are used:
- **MWBMPartners** (`github.com/MWBMPartners`) — parent company org. Hosts MeedyaDL main repo and MeedyaSuite-core (shared Rust crates).
- **MeedyaDL** (`github.com/MeedyaDL`) — project-specific org. Hosts MeedyaDL-Tools (dependency mirrors) and may host more project repos in future.

Both orgs are allowed in `deny.toml` via `[sources.allow-org] github = ["MWBMPartners", "MeedyaDL"]` for `cargo-deny` source checks.

**Why:** The project uses git dependencies from both orgs (MeedyaSuite-core from MWBMPartners, tool mirrors from MeedyaDL).

**How to apply:** When adding new git dependencies from either org, no `deny.toml` changes are needed. If a third org is introduced, add it to the `github` array.
