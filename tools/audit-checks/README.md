<!-- Copyright (c) 2026 MeedyaSuite. Licensed under the MIT License. -->

# Audit checks

Cross-source consistency checks for MeedyaDL. Each script validates that one
part of the codebase still agrees with another part that the Rust/TypeScript
compilers **cannot** check for you — the "code references something that
doesn't exist in another source" bug class.

They are invoked by the **PR Security Checks** workflow
(`.github/workflows/pr-security.yml`) on every pull request, and are runnable
locally with no dependencies beyond Python 3 (the TOML is parsed with
targeted regex, so no `tomllib`/`tomli`/venv is needed).

| Script | What it validates | Analogous bug class |
| --- | --- | --- |
| `check_ipc_commands.py` | Tauri IPC contract: every `#[tauri::command]` is registered in `lib.rs`'s `generate_handler![]`, and every frontend `invoke('x')` targets a registered command. | A button that calls a command the backend never registered → runtime "command not found". |
| `check_codec_registry.py` | `codecs.toml` integrity: every meta-codec `resolves_to` target is a real codec section, and every audio `services.gamdl` flag is a real `SongCodec` variant. | A renamed/removed codec leaving the registry pointing at nothing → download fails. |
| `check_user_agent.py` | Outbound User-Agent consistency: every `.header("User-Agent", ...)` / `.user_agent(...)` call site uses the shared `APP_USER_AGENT` constant (or the deliberate `APPLE_BROWSER_USER_AGENT`), never a hand-typed string literal. | A new call site hardcoding its own UA string, silently drifting out of sync with the app version (the MusicBrainz `"MeedyaDL/0.6"` defect this check exists to prevent recurring). |

## Running locally

```bash
# Advisory (always exits 0; prints any findings) — what a quick check looks like
python3 tools/audit-checks/check_ipc_commands.py
python3 tools/audit-checks/check_codec_registry.py
python3 tools/audit-checks/check_user_agent.py

# Strict (exits 1 on a high-severity finding) — handy in a pre-push hook
python3 tools/audit-checks/check_ipc_commands.py --strict
python3 tools/audit-checks/check_codec_registry.py --strict
python3 tools/audit-checks/check_user_agent.py --strict
```

## Conventions

- **Findings are printed as `  • path:line — message` bullets.** The
  workflow greps for the `•` bullet to decide whether to surface a section in
  the PR comment, so keep that prefix if you add findings.
- **Zero findings on a clean tree is mandatory.** These are precision tools,
  not lint nags — a check that cries wolf on day one gets ignored. Add a
  negative test (inject the drift, confirm it's caught, revert) when you add
  or change a check.
- **Default exit 0, `--strict` exit 1.** CI runs them advisory; local hooks
  can opt into blocking.

## Adding a check

Good candidates are pairs of sources that must agree but have no compiler
link between them. Ideas not yet implemented:

- `engines.toml` engine IDs ↔ the `EngineCommandBuilder` implementations
  registered in `engine_runner.rs`.
- `tool-versions.toml` tool IDs ↔ the tools `dependency_manager.rs` installs.
- Rust `AppSettings` fields ↔ the TypeScript `AppSettings` type (watch for
  serde renames — high false-positive risk; validate carefully before adding).
- i18n: keys referenced via `t('x')` ↔ keys present in
  `public/locales/en/translation.json`.
