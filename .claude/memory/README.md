# Shared Claude Memory

This directory holds **project-scoped** Claude memory files that we want every developer's Claude Code session to load automatically — RC milestone state, multi-service groundwork status, the macOS updater bug history, the GAMDL audit cadence, and similar project context that's not derivable from the code itself.

## How Claude memory works

Claude Code stores per-project memory in `~/.claude/projects/<sanitised-repo-path>/memory/`, with each fact in its own Markdown file (frontmatter + body) and a flat `MEMORY.md` index that points at them. The `<sanitised-repo-path>` is the absolute repo path with `/` replaced by `-`, so on every contributor's machine the directory is at a different location.

That per-machine path is why memory can't be loaded straight from the repo — Claude reads from a fixed home-directory location, not from inside the repo.

## What's in here

Only memory files of `type: project` are committed. **Personal memory** (`type: user`, `type: feedback`) lives only in your home directory and is never committed — the repo shouldn't dictate that every contributor's Claude session has to know about Lance's git workflow preferences or that he prefers force-with-lease over force-push.

If you want to make a piece of project context available to the whole team, drop it in this directory using the standard memory frontmatter format (see any existing file as a template) and add a one-line hook to [`MEMORY.md`](MEMORY.md).

## Bootstrap on a new dev machine

```sh
./scripts/sync-claude-memory.sh
```

The script computes the sanitised path for your clone, copies every Markdown file from `.claude/memory/` into your local `~/.claude/projects/.../memory/`, and merges the shared `MEMORY.md` hooks into your personal `MEMORY.md` (preserving any personal entries you've added).

Re-run after `git pull` whenever a teammate has updated a memory file. The copy is **one-way (repo → home)** — local edits to your home memory don't propagate back; if you want a change shared, edit the file in the repo and commit.

## Why one-way?

So the repo stays the deliberate source of truth. Local edits stay local until you copy them back into the repo and open a PR. This avoids the failure mode where a memory file silently drifts on one developer's machine and surprises everyone else when they sync.
