# Contributing to MeedyaDL

Thank you for your interest in contributing to MeedyaDL! This guide will help you get started.

## Development Setup

### Prerequisites

- **Node.js** -- whatever the current long-term-support (LTS) release is. CI installs `lts/*`, not a fixed version number, so there's no single number to pin to here.
- **Rust** -- a specific version pinned in [`src-tauri/rust-toolchain.toml`](src-tauri/rust-toolchain.toml), not "whatever stable is". `rustup` picks this up automatically once you're inside `src-tauri/`.
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
npm run type-check           # TypeScript type checking
npm run test                 # Run Vitest tests
npm run lint                 # ESLint
npm run format:check         # Prettier formatting check
npm run check:legal          # Licence-acknowledgement + upstream-licence checks (in src-tauri/deny.toml's spirit, but for Node deps too)
cargo check                  # Rust compilation check (in src-tauri/)
cargo test                   # Rust tests (in src-tauri/)
cargo clippy -- -D warnings  # Rust lints, treated as errors (what CI actually gates on)
python3 tools/audit-checks/check_help_topics.py     # every help page has a manifest line, every link resolves
python3 tools/audit-checks/check_ipc_commands.py    # every IPC command is registered and called correctly
python3 tools/audit-checks/check_codec_registry.py  # codec registry cross-references are consistent
```

There are more scripts under `tools/audit-checks/` than the three above -- they're the cross-source consistency checks that also run in CI's `pr-security.yml`. Each one exits 0 on a clean tree and prints `path:line — message` bullets when something's wrong.

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

## Adding or Translating a Help Page

Help pages live in `help/*.md` and nowhere else -- the in-app Help screen is built from those same files, so there is no second, hand-typed copy to keep in sync.

- **To change what a page says**: edit the file directly.
- **To add a new page**: add the Markdown file **and** one line to `HELP_TOPIC_MANIFEST` in `src/components/help/helpTopics.ts` (that line gives the page its label, icon, and place in the sidebar order). A page with a file but no manifest line, or a manifest line with no file, is caught by `python3 tools/audit-checks/check_help_topics.py`.
- **To translate a page**: add a file at `help/<language>/<same file name>.md` (for example, a German translation of `getting-started.md` is `help/de/getting-started.md`). No manifest change is needed for a translation -- the app looks up a translated file by name automatically. A page with no translation yet falls back to the English original with a note saying so.
- A few rules apply only to help pages, not the rest of this repo's Markdown: no GitHub-only syntax (an emoji shortcode like `:rocket:` shows up as literal text in the app, which has no emoji renderer -- use the real character or leave it out); a link to another help page is an ordinary relative link (`[Cookie Management](cookie-management.md)`) and becomes real in-app navigation, so the filename has to be exact; and the copyright comment at the top of every file is stripped before display, so it never needs updating for wording changes.
- `index.md` is the one file the app doesn't show -- it's the table of contents for someone reading the files on GitHub; the app's own sidebar already does that job.
- `check_help_topics.py` also catches a stray emoji shortcode, a broken deep link from the app's code to a page that doesn't exist, and a broken link from one help page to another.

## Coding Conventions

- **Copyright header**: Every source file starts with
  `// Copyright (c) 2024-2026 MeedyaSuite` and a line pointing at the MIT
  licence. The range starts at 2024 because that is when the first commit
  was made; `scripts/update-copyright-year.sh` moves the end of the range
  forward and can be run with `--dry-run` first to see what it would change.
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

MeedyaDL uses a four-tier release-channel ladder (least → most stable): `alpha → beta → release-candidate → main (stable)`.

(MeedyaDL used to also run three cron-driven channels below Alpha — Nightly, Weekly, Monthly — but they were removed in the v1.11.0 cleanup. The Alpha channel covers the same "give me the latest work-in-progress" need on its own now.)

All four channel branches (`alpha`, `beta`, `release-candidate`, `main`) are **long-lived and protected** against deletion and non-fast-forward pushes. The `Auto-Delete Merged Branches` workflow keeps merged `feat/*` / `fix/*` branches from accumulating but exempts the four protected ones.

- Open PRs against `main`. Your branch name should start with `feat/` or `fix/`.
- Pushing to `alpha`, `beta`, or `release-candidate` (a maintainer's direct push, or a merged PR) triggers that channel's own release build — see [DEV_NOTES.md → Release Channels](DEV_NOTES.md#release-channels) for the full pipeline and in-app update-channel guard.

## Pull Request Process

1. Create a feature branch from `main`: `git checkout -b feat/your-feature`
2. Make your changes with conventional commit messages
3. Ensure the checks CI will run actually pass: `npm run lint && npm run type-check && npm run test`, then from inside `src-tauri/`, `cargo clippy -- -D warnings && cargo test`. CI fails the pull request on any one of these.
4. Push and open a pull request against `main`
5. Link related GitHub Issues in the PR description (e.g., "Fixes #123")
6. Wait for CI to pass and a maintainer to review

> The merged PR branch is auto-deleted; you don't need to clean it up. Protected channel branches are never deleted.

## Reporting Issues

- **Bugs**: [open a new issue](https://github.com/MWBMPartners/MeedyaDL/issues/new) and say what you did, what you expected, and what happened instead. (There is no bug report form yet — only the crash report form the app itself uses.)
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
