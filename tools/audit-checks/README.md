<!-- Copyright (c) 2024-2026 MeedyaSuite. Licensed under the MIT License. -->

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
| `check_codec_registry.py` | `codecs.toml` integrity: every meta-codec `resolves_to` target is a real codec section, every audio `services.gamdl` flag is a real `SongCodec` variant, and every video `services.gamdl` flag is a real `VideoCodec` variant (each enum's own `rename_all` mode is read from the source, since `SongCodec` is kebab-case but `VideoCodec` is lowercase-with-no-dashes). | A renamed/removed codec leaving the registry pointing at nothing → download fails. |
| `check_user_agent.py` | Outbound User-Agent consistency: every `.header("User-Agent", ...)` / `.user_agent(...)` call site uses one of the four named identities in `utils/http_client.rs` — `APP_USER_AGENT`, `SAFARI_MACOS_USER_AGENT`, `browser_user_agent()` or `full_user_agent()` — never a hand-typed string literal. | A new call site hardcoding its own UA string, silently drifting out of sync with the app version (the MusicBrainz `"MeedyaDL/0.6"` defect this check exists to prevent recurring). |
| `check_tauri_version_sync.py` | The Tauri npm package and the Tauri Rust crate agree on major.minor, read from whatever `package-lock.json` and `Cargo.lock` are actually at this commit. | A version bump touching only one of the two lock files → `tauri build` refuses the mismatch and every platform build fails at once (the v1.10.5 incident). |
| `check_build_secrets.py` | Every build-time value the app reads — `option_env!("NAME")` in Rust, `import.meta.env.VITE_NAME` in the frontend — is either passed through by `release.yml` or listed in the script as deliberately not needed. | A finished feature shipping completely inert because its value was never wired into the release build. The app treats "absent" as "not configured" and says nothing, so nothing fails and nobody notices — three features were in exactly that state, none ever having worked once (#1161, #1162, #1163). |
| `check_help_topics.py` | Help docs: every `help/<id>.md` file has a line in `HELP_TOPIC_MANIFEST` (`helpTopics.ts`) and vice versa; every in-app deep link (`helpTopic="..."`, `navigateToHelp('...')`) and every help-page-to-help-page link points at a real page; no GitHub-only emoji shortcode (`:rocket:`) that would show as literal text in the app; every translated page has an English original. | The in-app Help and the help files used to be two hand-typed copies of the same words, kept in sync by hand — and #949 was the moment they disagreed somewhere a user could see it (the two copies named different "coming soon" versions). The hand-typed copy is gone, but a file and the app's list of pages are still two sources that have to agree. |
| `check_i18n.py` | Translation catalogue: every `public/locales/<lang>/translation.json` has exactly the same keys as `en`'s, no translated value is an empty string, and every `{{placeholder}}` in the English value is present in every translation. Also reports (informationally, not as a fault) how many keys nothing in `src/` looks up yet, and what fraction of `src/components/**/*.tsx` calls `useTranslation()`. | A key added in English only, or a translator's edit that drops a `{{count}}` or leaves a value blank, renders correctly in English and wrong (a raw key, a blank line, or a literal `{{count}}`) in every other language — the class of bug nobody on an English-language dev machine would ever see. |
| `check_comment_paths.py` | Every file path mentioned in a Rust/TypeScript/JavaScript/Python comment (starting with a real top-level directory of this repo — `src/`, `src-tauri/`, etc.) actually exists on disk. | A file gets renamed or moved and every comment that used to point at it keeps pointing at the old name forever — nothing about renaming a file touches the text of a comment sitting in some other file. An audit found eleven of these at once, none caught until someone happened to re-read the comment. |
| `check_concurrency_claims.py` | Every comment containing the word "parallel" or "concurrently" sits inside (or immediately above) a block of code that actually contains one of `join!`, `join_all`, `try_join`, `spawn`, `Promise.all`, `allSettled`. Deliberately a rough heuristic — see the script's own docstring — with a documented `EXCEPTIONS` list for claims that are true but whose mechanism lives elsewhere. | A comment claiming two things happen at the same time when the code actually awaits them one after another — in Rust, a future does nothing until it is polled, so building two futures and awaiting each in turn is sequential no matter what a comment says. An audit found five of these, including one where the futures actually were built together but then awaited one at a time immediately below. |
| `check_settings_reach_backend.py` | Every field in `pub struct AppSettings` that the app lets someone change (`useSettingsField('x')` or an `x:` key inside an `updateSettings({...})` call) is either read somewhere in `src-tauri/src` (a `.x` token outside `models/settings.rs` itself), or listed in the script's `FRONTEND_ONLY` dictionary with a checked, honest reason (the theme, the UI language, and other things genuinely acted on by the frontend alone). | A setting with a working UI control, that saves correctly, round-trips through the store perfectly, and changes nothing — because nothing downstream ever reads the value. This is exactly the shape the video "resolution fallback" list turned out to be: it looked exactly as meaningful as every setting next to it. |

## Running locally

```bash
# Advisory (always exits 0; prints any findings) — what a quick check looks like
python3 tools/audit-checks/check_ipc_commands.py
python3 tools/audit-checks/check_codec_registry.py
python3 tools/audit-checks/check_user_agent.py
python3 tools/audit-checks/check_tauri_version_sync.py
python3 tools/audit-checks/check_build_secrets.py
python3 tools/audit-checks/check_help_topics.py
python3 tools/audit-checks/check_i18n.py
python3 tools/audit-checks/check_comment_paths.py
python3 tools/audit-checks/check_concurrency_claims.py
python3 tools/audit-checks/check_settings_reach_backend.py

# Strict (exits 1 on a high-severity finding) — handy in a pre-push hook
python3 tools/audit-checks/check_ipc_commands.py --strict
python3 tools/audit-checks/check_codec_registry.py --strict
python3 tools/audit-checks/check_user_agent.py --strict
python3 tools/audit-checks/check_tauri_version_sync.py --strict
python3 tools/audit-checks/check_build_secrets.py --strict
python3 tools/audit-checks/check_help_topics.py --strict
python3 tools/audit-checks/check_i18n.py --strict
python3 tools/audit-checks/check_comment_paths.py --strict
python3 tools/audit-checks/check_concurrency_claims.py --strict
python3 tools/audit-checks/check_settings_reach_backend.py --strict
```

## Conventions

- **Findings are printed as `  • path:line — message` bullets.** The
  workflow greps for the `•` bullet to decide whether to surface a section in
  the PR comment, so keep that prefix if you add findings.
- **Print a `### ` heading line before the bullets.** This is not decoration,
  and getting it wrong is silent. `pr-security.yml` extracts a check's output
  with `grep -A100 '###'` before handing it to `add_section`, which skips an
  empty body without counting it. A script that prints bullets but no `###`
  line has **every finding discarded, with nothing reporting that it
  happened** — the check appears to run, passes, and tells you nothing.

  All four original scripts did this, but nobody had written it down, so the
  fifth (`check_build_secrets.py`) was added without one and was quietly
  useless until a review caught it. If you add a check, reproduce a real
  finding and confirm it survives the pipeline:

  ```bash
  OUT=$(python3 tools/audit-checks/your_check.py 2>&1)
  echo "$OUT" | grep -q '•' && [ -n "$(echo "$OUT" | grep -A100 '###')" ] \
    && echo "finding survives" || echo "finding would be DISCARDED"
  ```
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

(The i18n idea that used to be listed here — keys referenced via `t('x')` ↔
keys present in `public/locales/en/translation.json` — is implemented as
`check_i18n.py`, above.)
