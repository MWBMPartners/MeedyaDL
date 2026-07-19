# Contributing to MeedyaDL

Thank you for your interest in contributing to MeedyaDL! This guide will help you get started.

## Development Setup

### Prerequisites

- **Node.js** 20+ and npm
- **Rust** (stable toolchain) via [rustup](https://rustup.rs/)
- **Platform dependencies** for Tauri: see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

### Getting Started

```bash
# Clone the repository
git clone https://github.com/MWBMPartners/MeedyaDL.git
cd MeedyaDL

# Install npm dependencies
npm install

# Run the development server
npm run tauri dev

# Or run frontend-only (faster, no Rust compilation)
npm run dev
```

### Useful Commands

```bash
npm run type-check    # TypeScript type checking
npm run test          # Run Vitest tests
npm run lint          # ESLint
npm run format:check  # Prettier formatting check
cargo check           # Rust compilation check (in src-tauri/)
cargo test            # Rust tests (in src-tauri/)
```

### Disk-space hygiene (recommended)

A Tauri build produces a 20–40 GB `src-tauri/target/` directory. Combined with
`node_modules/` (~340 MB) and shared Cargo/npm caches (~4 GB), MeedyaDL's dev
workspace can claim 40+ GB of disk that's regenerable from source. This repo
ships two opt-in helpers in `scripts/`:

```bash
# One-shot cleanup — clears regenerable caches IF the script decides it's needed
./scripts/cleanup-after-pr.sh                # always clean
./scripts/cleanup-after-pr.sh --conditional  # only when free disk < 20 GB

# One-time setup: install a post-merge git hook that calls the script
# in --conditional mode every time you pull (i.e. after a PR merges)
./scripts/install-dev-hooks.sh

# Customise the threshold via env var (default 20 GB):
export MEEDYADL_CLEANUP_THRESHOLD_GB=40
# Add to ~/.zshrc to make it permanent
```

The hook is local (`.git/hooks/post-merge`), not committed. Each contributor
runs `install-dev-hooks.sh` once per clone. Re-running it is idempotent.

What gets cleaned: `src-tauri/target/`, `node_modules/`, Vite caches, Cargo
registry caches, npm cache, pip cache (macOS), Homebrew old versions. Never
touched: `.git/`, browser data, user app caches, APFS local snapshots.

## Project Structure

See [`.claude/CLAUDE.md`](.claude/CLAUDE.md) for a comprehensive architecture overview including:
- Key directories and their purpose
- Service/command/model relationships
- Feature implementation details

## Coding Conventions

- **Copyright header**: Every source file starts with `// Copyright (c) 2026 MeedyaSuite` + MIT licence reference
- **Comments**: Every function and significant code block gets detailed comments
- **Conventional commits**: Required for automated changelog generation
  - `feat:` — new feature
  - `fix:` — bug fix
  - `docs:` — documentation only
  - `refactor:` — code change that neither fixes a bug nor adds a feature
  - `perf:` — performance improvement
  - `test:` — adding or correcting tests
  - `chore:` — build process, CI, dependencies
  - `security:` — security-related changes

### Release notes

Every user-facing PR (title starts with `feat`, `fix`, or `perf`) must end its body with a `Release-Note:` line — one plain-English sentence per user-visible change, or `Release-Note: none` if there isn't one:

```text
Release-Note: Fixed wrapper connections for people running the wrapper on another computer while on an older GAMDL version.
```

This trailer becomes a commit footer at squash-merge time and is what turns MeedyaDL's release notes into something an end user can actually read — no file names, function names, or CLI flags, just what changed for them. `release-note-gate.yml` enforces its presence on CI. See [`.github/release-notes/STYLE_GUIDE.md`](.github/release-notes/STYLE_GUIDE.md) for the full writing guide, including worked before/after examples.

## Branching Model

MeedyaDL uses a six-tier release-channel ladder (least → most stable):

```
feat/* ─→ nightly ─→ weekly ─→ monthly ─→ alpha ─→ beta ─→ main (stable)
```

All six channel branches are **long-lived and protected** against deletion and non-fast-forward pushes. The `Auto-Delete Merged Branches` workflow keeps merged `feat/*` / `fix/*` branches from accumulating but exempts the six protected ones.

- Open PRs against `main`. Your branch name should start with `feat/` or `fix/`.
- The **Nightly Release** workflow (`nightly-release.yml`) automatically merges every `feat/*` branch into `nightly` daily at 00:00 UTC, tags `vX.Y.Z-nightly.YYYYMMDD`, and triggers a nightly build. If your branch conflicts with another, the workflow skips it and files an issue; rebase on `main` and the next nightly will pick it up.
- Weekly / monthly / alpha / beta integrate upward from the channel directly below on their own cadence.
- See [DEV_NOTES.md → Release Channels](DEV_NOTES.md#release-channels) for the full pipeline and in-app update-channel guard.

## Pull Request Process

1. Create a feature branch from `main`: `git checkout -b feat/your-feature`
2. Make your changes with conventional commit messages
3. Ensure all checks pass: `npm run type-check && npm run test`
4. Push and open a pull request against `main`
5. Link related GitHub Issues in the PR description (e.g., "Fixes #123")
6. Wait for CI to pass and a maintainer to review

> The merged PR branch is auto-deleted; you don't need to clean it up. Protected channel branches are never deleted.

## Reporting Issues

- **Bugs**: Use the [bug report template](https://github.com/MWBMPartners/MeedyaDL/issues/new?template=bug_report.md)
- **Crash reports**: Use the in-app crash reporting (Settings > Advanced > Error Reporting)
- **Feature requests**: Open a [discussion](https://github.com/MWBMPartners/MeedyaDL/discussions) or issue
- **Security vulnerabilities**: See [SECURITY.md](SECURITY.md) — do NOT open public issues

## Release Process

Releases are automated via [release-please](https://github.com/googleapis/release-please):
1. Conventional commits on `main` trigger a Release PR
2. Merging the Release PR creates a git tag
3. The tag triggers the Release workflow, building for all 6 platforms
4. Changelog is auto-generated by git-cliff

**Do not push directly to `main` expecting binaries** — the Release PR must be merged first.

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). Please read it before participating.
