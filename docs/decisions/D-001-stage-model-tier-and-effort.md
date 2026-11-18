# D-001 — Model tier and effort level for each stage of work

- **Date:** 2026-11-18
- **Question:** Which model tier and how much reasoning effort should each stage of work use?
- **Answer:** Gathering information: Sonnet or Haiku at Medium effort. Deep analysis and planning: Opus at High effort. Implementation: Sonnet or Haiku at Medium effort. Checks and reviews stay on Opus.
- **Rules out:** Running planning on a cheaper tier or at low effort; running routine building on Opus at high effort; reviewing with a model weaker than the one that built.
- **Applies to:** every LLM platform (Claude, Codex, Ollama, Kimi, Grok, Gemini, others), using each platform's own reasoning-level setting.
- **Implemented in:** `AGENTS.md`, `.claude/CLAUDE.md`, `.claude/memory/project_standing_rules.md` and its `.OpenAI` copy; device-wide `~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md`, and the master `Build Infrastructure/Config/standing-rules.md`.
- **Issue:** #1453
- **Supersedes:** nothing. It adds effort levels to the 16 Nov 2026 model-tier rule.
