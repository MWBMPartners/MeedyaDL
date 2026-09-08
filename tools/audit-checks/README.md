<!-- Copyright (c) 2026 MeedyaSuite. Licensed under the MIT License. -->

# Audit checks

Cross-source consistency checks for MeedyaDL. Each script validates that one
part of the codebase still agrees with another part that the Rust/TypeScript
compilers **cannot** check for you — the "code references something that
doesn't exist in another source" bug class.

`check_build_secrets.py` stretches that remit slightly: it compares the code
against the release workflow rather than against another piece of code. It
belongs here anyway, because it catches the same underlying shape — two things
that must agree, with nothing to notice when they stop.

They are invoked by the **PR Security Checks** workflow
(`.github/workflows/pr-security.yml`) on every pull request, and are runnable
locally with no dependencies beyond Python 3 (the TOML is parsed with
targeted regex, so no `tomllib`/`tomli`/venv is needed).

| Script | What it validates | Analogous bug class |
| --- | --- | --- |
| `check_ipc_commands.py` | Tauri IPC contract: every `#[tauri::command]` is registered in `lib.rs`'s `generate_handler![]`, and every frontend `invoke('x')` targets a registered command. | A button that calls a command the backend never registered → runtime "command not found". |
| `check_codec_registry.py` | `codecs.toml` integrity: every meta-codec `resolves_to` target is a real codec section, and every audio `services.gamdl` flag is a real `SongCodec` variant. | A renamed/removed codec leaving the registry pointing at nothing → download fails. |
| `check_user_agent.py` | Outbound User-Agent consistency: every `.header("User-Agent", ...)` / `.user_agent(...)` call site uses the shared `APP_USER_AGENT` constant (or the deliberate `APPLE_BROWSER_USER_AGENT`), never a hand-typed string literal. | A new call site hardcoding its own UA string, silently drifting out of sync with the app version (the MusicBrainz `"MeedyaDL/0.6"` defect this check exists to prevent recurring). |
| `check_tauri_version_sync.py` | The Tauri npm package and the Tauri Rust crate agree on major.minor, read from whatever `package-lock.json` and `Cargo.lock` are actually at this commit. | A version bump touching only one of the two lock files → `tauri build` refuses the mismatch and every platform build fails at once (the v1.10.5 incident). |
| `check_build_secrets.py` | Every build-time value the app reads — `option_env!("NAME")` in Rust, `import.meta.env.VITE_NAME` in the frontend — is either passed through by `release.yml` or listed in the script as deliberately not needed. | A finished feature shipping completely inert because its value was never wired into the release build. The app treats "absent" as "not configured" and says nothing, so nothing fails and nobody notices — three features were in exactly that state, none ever having worked once (#1161, #1162, #1163). |

## Running locally

```bash
# Advisory (always exits 0; prints any findings) — what a quick check looks like
python3 tools/audit-checks/check_ipc_commands.py
python3 tools/audit-checks/check_codec_registry.py
python3 tools/audit-checks/check_user_agent.py
python3 tools/audit-checks/check_tauri_version_sync.py
python3 tools/audit-checks/check_build_secrets.py

# Strict (exits 1 on a high-severity finding) — handy in a pre-push hook
python3 tools/audit-checks/check_ipc_commands.py --strict
python3 tools/audit-checks/check_codec_registry.py --strict
python3 tools/audit-checks/check_user_agent.py --strict
python3 tools/audit-checks/check_tauri_version_sync.py --strict
python3 tools/audit-checks/check_build_secrets.py --strict
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
