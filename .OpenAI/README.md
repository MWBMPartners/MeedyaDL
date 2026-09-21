# OpenAI / Codex project context

This directory is a repository-local copy of the shared project context and
project-scoped memory used by AI-assisted development environments.

- `CONTEXT.md` describes the architecture, conventions, and current subsystems.
- `PROJECT_BRIEF.md` contains the broader project brief and historical context.
- `memory/` contains shared project facts and handoff notes.

Only shared project information belongs here. Do not add API keys, credentials,
local permission settings, chat transcripts, personal preferences, or other
machine/user-specific data. `.claude/memory/` remains the canonical Claude memory location; this copy makes the
same context available to Codex and other OpenAI-powered tools. Codex does not look in
this folder by itself: it reads files named `AGENTS.md` (and a few other names only if set
up to). The repository-root
`AGENTS.md` (added 2026-09-21, #1198) is what points it here, so keep that file in step
with the standing rules.
