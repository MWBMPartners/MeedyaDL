---
name: project-audit-checks-inventory
description: The nine cross-source consistency scripts in tools/audit-checks/ — what each one catches, the house rules they all follow, and the pipe-swallows-your-findings trap every new one must be proven against
metadata:
  type: project
---

# The nine scripts in `tools/audit-checks/`

Each script checks that one part of the codebase still agrees with another
part — the kind of thing that no compiler catches, because both sides are
individually valid code or valid text; they have just quietly stopped
matching each other. They run in `pr-security.yml` on every pull request
and are runnable locally with nothing beyond Python 3's standard library.

## What each one catches

- **`check_build_secrets.py`** — every build-time value the code reads
  (`option_env!("NAME")` in Rust, `import.meta.env.VITE_NAME` in the
  frontend) is either passed through by `release.yml` or named on a
  deliberate "not needed" list in the script itself. This is the check
  that would have caught the three inert features in
  [[project-never-worked-pattern]] before they shipped.
- **`check_help_topics.py`** — every file in `help/*.md` has a line in
  `HELP_TOPIC_MANIFEST` and vice versa, every in-app deep link to a help
  page and every help-page-to-help-page link points at a real page, no
  GitHub-only emoji shortcode that would show as literal text in the app,
  and every translated help page has a real English original.
- **`check_i18n.py`** — every language file under `public/locales/` has
  exactly the same keys as English, no translated value is empty, and
  every `{{placeholder}}` token in the English value is present in the
  translation. Also reports, informationally rather than as a fault, how
  many translation keys nothing in `src/` looks up yet.
- **`check_comment_paths.py`** — every file path named in a comment
  actually exists on disk. See [[project-comment-accuracy-hazard]].
- **`check_concurrency_claims.py`** — every comment claiming two things
  happen "in parallel" or "concurrently" sits near code that actually
  contains a mechanism that could make that true. A deliberately rough
  heuristic with a documented exceptions list — see
  [[project-comment-accuracy-hazard]].
- **`check_ipc_commands.py`** — every Rust `#[tauri::command]` is
  registered in `lib.rs`'s `generate_handler![]`, and every frontend
  `invoke('x')` call names a command that is actually registered. Catches
  the "compiles fine, fails at runtime with command not found" class.
- **`check_codec_registry.py`** — every meta-codec's `resolves_to` target
  in `codecs.toml` is a real concrete codec section, and every concrete
  codec's `services.gamdl` value is a real `SongCodec` Rust enum variant.
  Catches a renamed or removed codec left pointing at nothing.
- **`check_user_agent.py`** — every outbound HTTP request sets its
  User-Agent through one of the four named constants/functions in
  `http_client.rs`, never a hand-typed string literal that could silently
  drift out of sync with the app's own version number.
- **`check_tauri_version_sync.py`** — the Tauri npm package and the Tauri
  Rust crate agree on major.minor version, reproducing the exact check the
  Tauri CLI itself runs (and refuses to build past if it fails).

## House rules every script in this directory follows

- **Standard library only.** No `tomllib`/`tomli`/venv — TOML gets parsed
  with targeted regex rather than pulling in a dependency for one read.
- **Findings print as `  • path:line — message` bullets**, because the
  workflow greps for the `•` character to decide whether a section has
  anything to show.
- **Exit 0 normally, exit 1 under `--strict`.** CI runs every script in
  advisory mode; a local pre-push hook can opt into the blocking form.
- **A named dictionary of deliberate exceptions**, each entry carrying a
  reason a person reading the script later can act on — never a bare list
  of strings to silence with no explanation of why they are there.
- **Zero findings on a clean tree is mandatory.** These are precision
  tools, not lint nags that get ignored after crying wolf once. Any change
  to a check needs a negative test alongside it: inject the drift the
  check exists to catch, confirm it is caught, then revert the injected
  drift.

## The trap every new check must be proven against

**A check must print a `### ` heading line, or the workflow's
`grep -A100 '###'` silently discards every finding it produced.** This has
already happened once in this project: `check_build_secrets.py` shipped
without the heading line, ran in CI, found nothing wrong with itself, and
was quietly useless — passing every run while reporting nothing — until a
review caught the missing heading by reading the script rather than
trusting its green result.

Because of that history, **every new check must be proven against this
exact failure before it is considered finished**: deliberately introduce
the fault the check exists to catch, run the check, and confirm the
finding survives the `grep -A100 '###'` pipe the workflow actually uses —
not just that the script prints something when run directly.

```bash
OUT=$(python3 tools/audit-checks/your_check.py 2>&1)
echo "$OUT" | grep -q '•' && [ -n "$(echo "$OUT" | grep -A100 '###')" ] \
  && echo "finding survives" || echo "finding would be DISCARDED"
```

## The two guards that live as tests, not scripts

Two accessibility/design guards for the frontend live as ordinary Vitest
test files rather than standalone audit scripts, because what they check
is naturally a runtime assertion rather than a static grep:

- **`src/lib/tailwindColorClasses.test.ts`** — builds the real Tailwind CSS
  output the way `npm run build` does and fails if any component uses a
  colour class name that compiles to no rule at all (a colour nobody ever
  defined). This is how it tells a genuine typo from a legitimate
  non-colour utility that happens to share a prefix, like `bg-cover`.
- **`src/styles/themes/textOnFillContrast.test.ts`** — reads every theme's
  real CSS, finds every place a fill colour and its paired text colour are
  declared together, and checks the WCAG contrast ratio between them holds
  in every one of the six themes, not just the one a developer happened to
  be looking at while writing the code.

Full details, the running commands, and the "ideas not yet implemented"
list are in `tools/audit-checks/README.md`.

Related: [[project-never-worked-pattern]], [[project-comment-accuracy-hazard]],
[[project-codebase-sweeps-2026-09]], [[project-pr-security-checks]]
