---
name: PR security heuristics + audit-checks (pr-security.yml, tools/audit-checks/)
description: How MeedyaDL's per-PR security heuristic gate and cross-source consistency scripts work, why they're shaped the way they are, and the gotchas hit standing them up — adapted from WebMS-Intra to the Rust/TS/Tauri stack
type: project
---
Landed 2026-06-03 on PR #905 (`ci: add PR security heuristics workflow + cross-source audit checks`), CI 12/12 green. Adapted from the WebMS-Intra `pr-security.yml` approach at the user's request ("add similar PR heuristics security checks to this project"). WebMS-Intra is PHP; MeedyaDL is Rust + TypeScript + Tauri, so this is an **adaptation, not a port** — the PHP-specific checks (PHP lint hard gate, mysqli SQL-injection, CSRF tokens, Psalm) were dropped and the heuristic layer re-aimed at MeedyaDL's own documented invariants.

## What exists

- **`.github/workflows/pr-security.yml`** — runs on PRs to `main` / `release-candidate` / `beta` / `alpha` + `workflow_dispatch`. **Every check is non-blocking (`continue-on-error`)** — the merge gate stays with `ci.yml` (clippy `-D warnings`, cargo test, cargo-deny, tsc, eslint, CodeQL). This adds only the heuristic layer those gates don't cover.
- **`tools/audit-checks/check_ipc_commands.py`** + **`check_codec_registry.py`** + **`README.md`** — zero-dependency Python cross-source validators, runnable locally.
- **`.github/pull_request_template.md`** — manual security-review checklist mapped to MeedyaDL invariants.
- **`.claude/CLAUDE.md`** — convention bullet under the "Licence compliance is enforced per-PR" neighbour.

## The 8 workflow checks

1. gitleaks CLI secrets scan (working tree + commits since base, `--redact`); findings surfaced in the PR comment, **no SARIF upload** (keeps perms at `contents:read` + `pull-requests:write`). 2. Rust subprocess shell-interpolation (`Command::new("sh")` / `.arg("-c")` — the "no `sh -c`" rule). 3. `unsafe` Rust in non-test changed code. 4. Dangerous frontend sinks (`eval` / `new Function` / `dangerouslySetInnerHTML` / `innerHTML=`). 5. Hardcoded absolute paths (`/Users/`, `/home/<user>/`, `C:\`). 6. Unpinned GitHub Actions (`uses: org/repo@<tag>` not a 40-hex SHA — handles both `- uses:` and bare `uses:` forms; exempts `./local` and `docker://`). 7. Sensitive/proprietary path touches (`assets/brand/` is PROPRIETARY, `src-tauri/capabilities/`, `tauri.conf.json`, `.github/workflows/`, signing/entitlements). 8. The two consistency scripts.

Checks 2–7 scan only PR-changed files; check 8 validates whole-repo state.

## The two consistency scripts (the WebMS checks-9-11 analog)

Both follow the WebMS pattern: validate that two sources which must agree have no compiler link between them ("code references something that doesn't exist in another source").

- **`check_ipc_commands.py`** — every `#[tauri::command]` under `src-tauri/src/` is registered in `lib.rs`'s `generate_handler![]`, AND every frontend `invoke('x')` literal targets a registered command. Catches the runtime "command not found" class (defined-but-unregistered compiles fine; a typo'd invoke target only fails when a user clicks the button).
- **`check_codec_registry.py`** — every `codecs.toml` meta-codec `resolves_to` target is a real concrete codec section, AND every audio `services.gamdl` flag is a kebab-case `SongCodec` variant. TOML is parsed with **targeted regex, not `tomllib`** (so the script is Python-version-agnostic and venv-free).

**Conventions to preserve:**
- Findings print as `  • path:line — message` bullets; the workflow greps for the `•` bullet to decide whether to surface a section. Keep that prefix.
- **Zero findings on a clean tree is mandatory.** Both were verified clean on the current tree and negative-tested (inject a bad `invoke()`, a dangling `resolves_to`, a bogus `gamdl=` → all caught). Add a negative test when you change a check — a check that cries wolf on day one gets ignored.
- Default exit 0; `--strict` exits 1 on a high-severity finding (for local pre-push hooks).

The README lists good next candidates: `engines.toml` ↔ `EngineCommandBuilder` impls, `tool-versions.toml` ↔ installed tools, i18n `t('key')` ↔ locale JSON. (A Rust↔TS `AppSettings` drift check is tempting but high-false-positive because of serde renames — validate carefully before adding.)

## Design choices worth remembering

- **One upserted comment, not one per push.** The comment carries a hidden marker `<!-- meedyadl-pr-security -->`; the workflow finds an existing marked comment via `gh api …/issues/{n}/comments` and PATCHes it (`jq -Rs '{body: .}' | gh api --input -`), else POSTs a new one. WebMS posts a fresh comment every push; this avoids that spam.
- **Self-referential advisory is by design.** Check 7 flags any PR touching `.github/workflows/`, so the PR that *added* `pr-security.yml` got flagged by `pr-security.yml` — correct behaviour (reviewers should confirm workflow/brand changes are deliberate), advisory, no action.

## Gotchas hit standing it up (2026-06-03)

- **`actions/setup-python@e348410041c5b0ca4452c8e292ca3936bac9ba7f # v6` is NOT a resolvable SHA.** My first pr-security.yml copied this pin from `upstream-gamdl-watch.yml:60` (a repo grep "confirmed" it was already in use). The job died in 2 s at action-resolution: *"Unable to resolve action … unable to find version e348410…"*. Fix: the audit scripts are stdlib-only (`re`/`sys`/`pathlib`), so **`setup-python` was removed entirely** — ubuntu runners' preinstalled `python3` runs them. **`upstream-gamdl-watch.yml` still carries the same bad pin** and will fail at its Python-setup step whenever that cron fires — a latent bug worth a follow-up (flagged to the user 2026-06-03).
- **Detecting CI *success* from a remote session is awkward.** Webhooks deliver CI *failures* and comments but never success/new-push/merge-conflict transitions. Unauthenticated `api.github.com` polling hits the shared-runner-IP rate limit fast (60/hr), there was no `GH_TOKEN` in the shell, and `send_later` wasn't available. The working pattern: rely on the failure-webhook for interim breakage, and arm a `Monitor` single-shot timer (`sleep N && echo`) to wake the session and re-query check-runs via the **authenticated GitHub MCP** (`pull_request_read get_check_runs`) once. Re-arm if still running.
