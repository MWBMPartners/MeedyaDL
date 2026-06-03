<!--
MeedyaDL — Pull Request Template

The "PR Security Checks" workflow (.github/workflows/pr-security.yml) runs
automated heuristic + cross-source-consistency scans on every PR and posts a
single advisory comment. This checklist is your MANUAL review pass — the part
a grep can't do. Tick each row consciously; delete sections that genuinely
don't apply.
-->

## Summary

<!-- 1-3 sentences: what does this PR do and why. Link the issue it closes. -->

## Scope of changes

- [ ] Rust backend (`src-tauri/src/`)
- [ ] React / TypeScript frontend (`src/`)
- [ ] IPC surface (new/changed `#[tauri::command]`)
- [ ] Settings schema (`models/settings.rs` + TS types)
- [ ] Engine / codec / tool config (`engines.toml`, `codecs.toml`, `tool-versions.toml`, `tags.toml`)
- [ ] CI / workflows (`.github/`)
- [ ] Documentation (README, CHANGELOG, CLAUDE.md, `Project_Plan.md`, `help/*.md`)
- [ ] Other: _____

## Test plan

- [ ] `cargo test` (in `src-tauri/`) passes
- [ ] `npm run type-check` and `npm run test` pass
- [ ] `cargo clippy -- -D warnings` is clean
- [ ] Tested the golden path manually (`cargo tauri dev`)
- [ ] Tested at least one edge / failure case

## Security review

**Tick each row consciously.** These mirror MeedyaDL's documented invariants
(see `.claude/CLAUDE.md`). The automated workflow flags some of these
heuristically, but a clean bot comment is not a substitute for this pass.

### Input handling

- [ ] URLs are validated as `http(s)://` before reaching a subprocess (`gamdl_service.rs` guard), and download URLs are domain-allowlisted (Apple Music / Classical / iTunes)
- [ ] Any new filesystem path goes through `validate_path_safe()` (rejects `..` traversal); no path is built from unsanitised user input
- [ ] User strings written to GAMDL `config.ini` pass through `sanitize_ini_value()` (strips `\n` / `\r`)
- [ ] Imported settings/manifests are length-capped and control-char-stripped (`sanitize_imported_settings()`)

### Subprocess safety

- [ ] All subprocess calls use parameterised `Command::new().arg()` — **no** `sh -c` / `bash -c` / `format!()`-into-shell patterns
- [ ] No user input reaches `eval` / `new Function` / a shell

### Output & UI

- [ ] No `dangerouslySetInnerHTML` / `innerHTML =` of untrusted or remote-derived strings; Markdown is rendered through `rehype-sanitize`
- [ ] No raw secret/credential values are rendered into the DOM or logged to the activity log

### Secrets / credentials

- [ ] No API keys, developer tokens, `.p8` keys, passwords, or wrapper auth tokens are committed (embedded keys come from `option_env!` build secrets only)
- [ ] Sensitive values are stored in the OS keychain (`keyring`), not in `settings.json`
- [ ] Wrapper URLs are passed through `redact_url_query()` before any logging
- [ ] No new file was added under a server/secret-managed path without justification

### Filesystem

- [ ] No hardcoded absolute paths (`/Users/…`, `/home/<user>/…`, `C:\…`) — paths derive from `app_data_dir` / `std::env::temp_dir()`
- [ ] New on-disk writes that must survive a crash use the atomic temp-then-rename pattern

### IPC contract

- [ ] Every new `#[tauri::command]` is registered in `tauri::generate_handler![]` in `lib.rs` **and** has a frontend wrapper (`src/lib/tauri-commands.ts`)
- [ ] Rate-limited commands (downloads, update checks, cookie imports) keep their limiter
- [ ] `python3 tools/audit-checks/check_ipc_commands.py` is clean

### Settings / registry consistency

- [ ] If `AppSettings` changed: `settings_version` bumped + `migrate_settings()` updated, and the TypeScript type mirror updated
- [ ] If `codecs.toml` changed: `python3 tools/audit-checks/check_codec_registry.py` is clean

### Dependencies / licensing

- [ ] New dependencies are permissively licensed (cargo-deny allowlist) and named in `ACKNOWLEDGEMENTS.md` (`npm run check:legal` passes)
- [ ] No GPL/copyleft code is *linked* into MeedyaDL's own MIT code (subprocess invocation is fine)

### CI / supply chain

- [ ] New GitHub Actions are pinned to an immutable 40-char commit SHA (not a `@vX` tag)
- [ ] No `[skip ci]` in commit messages (unless explicitly requested)
- [ ] `workflow_dispatch` inputs are consumed via `env:`, not interpolated directly into `run:` shell

### Proprietary assets

- [ ] `assets/brand/` files (if touched) keep their **proprietary** license headers — these are NOT MIT

## Documentation

- [ ] Updated all affected docs (README, CHANGELOG, CLAUDE.md, `Project_Plan.md`, `help/*.md`) — feature lists, settings, commands, file counts, structure trees

## Related issues

<!-- Closes #N, refs #N. Per repo convention every change has a tracking issue. -->

---

<sub>Checklist enforced by repo convention, not by CI. The automated **PR Security Checks** workflow adds heuristic scans on top of this manual review; both are advisory — the merge gate is `ci.yml`.</sub>
