# OpenAI / Codex project context

This directory is a repository-local copy of the shared project context and
project-scoped memory used by AI-assisted development environments.

- `CONTEXT.md` describes the architecture, conventions, and current subsystems.
- `PROJECT_BRIEF.md` contains the broader project brief and historical context.
- `memory/` contains shared project facts and handoff notes.

Only shared project information belongs here. Do not add API keys, credentials,
local permission settings, chat transcripts, personal preferences, or other
machine/user-specific data. `.claude/memory/` remains the existing canonical
Claude memory location; this copy makes the same context discoverable in Codex
and other OpenAI-powered development environments.
