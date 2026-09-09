#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Translation catalogue consistency check.

MeedyaDL's UI text lives in `public/locales/<lang>/translation.json` --
one file per language declared in `LOCALES` (`src/lib/i18n.ts`), read by
`useTranslation()`'s `t('some.key')` calls throughout `src/`. Three things
have to agree for a translated screen to work, and nothing in TypeScript
checks any of them:

  (A) The English file      -- `public/locales/en/translation.json`, the
                               language every other file is a translation OF.
  (B) Every OTHER language file -- must have exactly the same set of keys
                               as English, with a real (non-empty) value,
                               and the same `{{placeholder}}` tokens.
  (C) The code that calls `t(...)` -- under `src/`.

Four things go wrong when they drift, and this script checks the first
three as real faults, then reports a fourth as information:

  1. A language file is missing a key English has, or has one English
     doesn't. This is exact and it means a screen renders the raw key
     ("crashReporting.title") instead of words, in that language only --
     the kind of bug nobody on an English-language dev machine would ever
     see.

  2. A translated value is an empty string. It renders as nothing --
     visually indistinguishable from a bug, not from "not translated yet".

  3. English's value has a `{{placeholder}}` (or several) and a
     translation is missing one. i18next just leaves the literal
     `{{count}}` sitting in the rendered sentence -- again, only visible
     to someone reading that language.

  4. A key is defined in English but never looked up anywhere in `src/`.
     This is NOT a fault -- MeedyaDL writes translation keys ahead of the
     component that will use them, deliberately, and about two thirds of
     the catalogue is sitting in exactly that state right now. It is
     reported as a plain count and list, not as a `•` finding, and it
     never affects the exit code. See "Why #4 doesn't use bullets" below.

What this script deliberately does NOT do: it does not look at `.tsx`
files and guess which hardcoded English strings on screen "ought" to be
translated. That would be pure guesswork -- flooding the output with
false alarms on class names, test ids, code comments, and genuine
non-text props -- and a check that cries wolf gets ignored, which is
worse than no check. This script only ever compares the translation
files against each other, and against the *code that already calls*
`t(...)` -- both are facts, not guesses.

## Dynamic key lookups

Two places in the app build a translation key at runtime instead of
writing `t('literal.key')` -- `Sidebar.tsx`'s `` t(`nav.${item.page}`) ``
(one key per sidebar page) and `SettingsPage.tsx`'s two similar lookups
for `settings.tabs.*` / `settings.groups.*`. A naive scan for quoted
`t('...')` calls would never see these and would wrongly report every key
under `nav.*` / `settings.tabs.*` / `settings.groups.*` as unused, on
literally the first run of this script.

Rather than hardcode that list of three namespaces (which would silently
go stale the next time someone writes a fourth dynamic lookup),
`collect_used_keys_and_dynamic_prefixes()` scans for the *shape* of the
pattern -- a template literal whose static prefix ends in `.` right
before `${` -- and treats every key under a namespace found that way as
reachable. See that function (and `DYNAMIC_PREFIX_RE`) for the regex and
its reasoning.

## Why #4 (unused keys) doesn't use `•` bullets

Every other check in this script, and every check in this directory,
prints a real finding as a `  • path:line — message` bullet, because
`pr-security.yml` greps the *whole script's output* for a `•` character
to decide whether to surface a PR comment section at all. That is exactly
right for a fault that should be zero on a healthy tree.

It would be exactly wrong for #4. On a healthy MeedyaDL tree, dozens of
keys are unused *by design* -- written ahead of the screen that will use
them -- and that number will not reach zero for a long time. Bulleting
all of them would mean this check permanently triggers a PR comment
section on every single PR, for a condition that isn't a problem. That is
precisely the "cries wolf on day one, gets ignored" failure this
directory's own README warns about. So #4 (and the "how much of the UI is
wired up" summary) print as plain, unbulleted lines -- informative when
you run this locally, invisible to the bullet-triggered PR comment.

Exit code:
  0 -- no findings in checks 1-3 (or findings without --strict)
  1 -- at least one finding in checks 1-3 AND --strict was passed

Usage:
  python3 tools/audit-checks/check_i18n.py [--strict]
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
LOCALES_DIR = REPO_ROOT / "public" / "locales"
I18N_TS = REPO_ROOT / "src" / "lib" / "i18n.ts"
FRONTEND_SRC = REPO_ROOT / "src"
COMPONENTS_DIR = FRONTEND_SRC / "components"

# The language every translation is a translation OF. Its file is the
# reference every other language file is measured against.
REFERENCE_LOCALE = "en"

# CLDR/i18next plural-form suffixes. `t('x.y', { count })` resolves to
# `x.y_one` / `x.y_other` / etc at runtime -- the base key `x.y` is never
# itself a key in the catalogue -- so a leaf like `x.y_one` needs its
# suffix stripped before it can be compared against what the source code
# actually calls. Used by `is_reachable()` to accept the un-suffixed base
# as evidence a suffixed leaf is used.
PLURAL_SUFFIXES = ("_zero", "_one", "_two", "_few", "_many", "_other")

# Keys that are deliberately never looked up via `t(...)` anywhere in the
# app, with the reason they are staying that way. Every entry here is
# still counted and listed in the informational "unused" report (#4) --
# this dictionary only changes how it's described, not whether it's
# reported -- because the point of #4 is an honest count, not a hidden
# allowlist.
#
# The rule for adding one, same as this project's other audit scripts:
# say why it is staying, in a sentence someone else could act on.
UNUSED_KEY_EXCEPTIONS: dict[str, str] = {
    "app.name": (
        "The product name is never looked up through i18next -- MeedyaDL is "
        "written out literally everywhere it appears on screen, per this "
        "project's rule that product and service names are never "
        "translated. This key is a structural placeholder (it mirrors "
        "`app.subtitle`, which IS looked up), not a translation gap."
    ),
}


def strip_line_comments(text: str) -> str:
    """Strip // line comments and /* */ block comments while preserving
    line numbers (block comments are replaced by an equal count of
    newlines so reported line numbers still line up with the original
    file). Copied from check_ipc_commands.py / check_help_topics.py rather
    than imported, to keep each audit script runnable on its own."""
    text = re.sub(
        r"/\*.*?\*/",
        lambda m: "\n" * m.group(0).count("\n"),
        text,
        flags=re.DOTALL,
    )
    text = re.sub(r"//[^\n]*", "", text)
    return text


def extract_bracket_block(text: str, opener: str) -> tuple[str, int]:
    """Find `opener` (e.g. "LOCALES = [") and bracket-match from its `[`
    to the matching `]`. Same technique `check_help_topics.py` uses to
    read `LOCALES` and `HELP_TOPIC_MANIFEST` -- this script does not
    attempt to parse TypeScript for real. Returns (block_text,
    offset_of_block_start)."""
    start = text.find(opener)
    if start == -1:
        return "", -1
    open_idx = text.index("[", start)
    depth = 0
    end_idx = open_idx
    for idx in range(open_idx, len(text)):
        c = text[idx]
        if c == "[":
            depth += 1
        elif c == "]":
            depth -= 1
            if depth == 0:
                end_idx = idx
                break
    return text[open_idx + 1 : end_idx], open_idx + 1


# ---------------------------------------------------------------------------
# (A) / (B): the locale files themselves.
# ---------------------------------------------------------------------------


def collect_locale_codes() -> list[str]:
    """Every language code the app knows about, read from `LOCALES` in
    `src/lib/i18n.ts` -- the one place (per that file's own comment) a
    supported language is declared. Falls back to whatever directories
    exist under `public/locales/` if `i18n.ts` can't be parsed, so this
    script degrades gracefully rather than silently checking zero
    languages."""
    if I18N_TS.exists():
        text = I18N_TS.read_text(encoding="utf-8", errors="ignore")
        block, block_offset = extract_bracket_block(text, "LOCALES = [")
        if block_offset != -1:
            stripped = strip_line_comments(block)
            codes = [
                m.group(1) for m in re.finditer(r"""code:\s*['"]([a-z]{2,3})['"]""", stripped)
            ]
            if codes:
                return codes
    print(f"WARNING: could not read LOCALES from {I18N_TS}; falling back to disk", file=sys.stderr)
    return sorted(p.name for p in LOCALES_DIR.iterdir() if p.is_dir())


def flatten(obj, prefix: str = "") -> dict[str, str]:
    """Flatten a nested translation JSON object into {dotted.key: value}.
    Only leaf string values are kept -- a translation file has no other
    shape (every branch is either an object or a final string)."""
    out: dict[str, str] = {}
    for k, v in obj.items():
        full = f"{prefix}.{k}" if prefix else k
        if isinstance(v, dict):
            out.update(flatten(v, full))
        else:
            out[full] = v
    return out


def load_locale(code: str) -> tuple[dict[str, str], str | None]:
    """Load and flatten one locale's translation.json. Returns
    (flattened_dict, error_message). error_message is None on success --
    a missing file or invalid JSON is reported as its own finding rather
    than crashing the script, since a translator's in-progress edit is
    exactly the kind of thing this check should catch cleanly."""
    path = LOCALES_DIR / code / "translation.json"
    if not path.exists():
        return {}, f"{path} does not exist"
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        return {}, f"{path} is not valid JSON: {e}"
    return flatten(data), None


# ---------------------------------------------------------------------------
# Placeholder tokens, e.g. "{{count}}", "{{tool}}".
# ---------------------------------------------------------------------------

PLACEHOLDER_RE = re.compile(r"\{\{\s*([a-zA-Z][a-zA-Z0-9_]*)\s*\}\}")


def placeholders_in(value: str) -> set[str]:
    return set(PLACEHOLDER_RE.findall(value))


# ---------------------------------------------------------------------------
# (C): the code that calls t(...).
# ---------------------------------------------------------------------------

# Any quoted, dot-separated identifier path, e.g. 'statusBar.actions.restart'
# or the more familiar 'sidebar.ready' inside a literal `t('sidebar.ready')`
# call. Deliberately NOT anchored to sit right after `t(` -- a key doesn't
# only reach `t(...)` by being typed directly inside the call.
# `StatusBar.tsx`'s `AFTER_QUEUE_LABEL_KEYS` is a real example already in
# this codebase: a `Record<string, string>` whose VALUES are key strings,
# looked up into a variable and only THEN passed to `t(labelKey)`. Anchoring
# to `t(` would miss that indirection entirely; this doesn't, because it
# doesn't care where the quoted string sits, only what it says.
#
# The false-positive risk this trades in return -- some unrelated quoted
# string coincidentally matching a real key's exact text -- is bounded to
# near zero below, where this is intersected against the actual set of
# reference-locale keys (or their plural bases): a match only counts if the
# literal text exactly equals a real "namespace.namespace.leafName"-shaped
# key, not merely something that looks key-shaped.
DOTTED_LITERAL_RE = re.compile(
    r"""['"]([a-zA-Z][a-zA-Z0-9_]*(?:\.[a-zA-Z][a-zA-Z0-9_]*)+)['"]"""
)

# The shape of a dynamically-built key: a template literal whose *static*
# text ends in a dot immediately before an interpolation, e.g.
# `` `nav.${item.page}` `` or `` `settings.tabs.${tabTranslationKey(id)}` ``.
# Captures the dotted prefix (with its trailing dot). Deliberately does
# NOT match a translated sentence that happens to contain "${...}" (e.g.
# `` `Keyboard shortcut: ${modifierKey} plus K` ``) because that text has
# a space before the colon, not an identifier immediately followed by a
# literal ".": the required shape is `` `word.word.${ `` with no other
# punctuation, which ordinary English prose does not produce by accident.
DYNAMIC_PREFIX_RE = re.compile(r"`((?:[a-zA-Z][a-zA-Z0-9_]*\.)+)\$\{")


def is_scannable_source_file(path: Path) -> bool:
    rel = str(path.relative_to(REPO_ROOT)).replace("\\", "/")
    if ".test." in path.name or "/test/" in rel or "__tests__" in rel:
        return False
    return True


def collect_used_keys_and_dynamic_prefixes() -> tuple[set[str], set[str]]:
    """Scan every non-test .ts/.tsx file under src/ once, for:

      - every quoted dotted-path string at all (`all_dotted_literals`,
        UNFILTERED -- see DOTTED_LITERAL_RE's comment for why this is
        broader than "inside a t(...) call" on purpose, and
        `is_reachable()` below for how it's actually used: a leaf key
        counts as reachable if the leaf's exact text, OR its plural
        base with `_one`/`_other`/etc stripped, shows up as one of
        these candidates -- filtering against the real key set happens
        at the comparison point, not here, precisely so the plural-base
        check has something to compare against even though a plural
        base like "sidebar.updatesAvailable" is never itself a real
        leaf key), and
      - the dynamic-key-lookup *shape* (`dynamic_prefixes`), e.g. "nav."

    Doing both in one pass over the same file list keeps this from
    reading every source file more than once.
    """
    all_dotted_literals: set[str] = set()
    dynamic_prefixes: set[str] = set()

    for ext in ("*.ts", "*.tsx"):
        for f in sorted(FRONTEND_SRC.rglob(ext)):
            if not is_scannable_source_file(f):
                continue
            try:
                raw = f.read_text(encoding="utf-8", errors="ignore")
            except OSError:
                continue
            stripped = strip_line_comments(raw)

            for m in DOTTED_LITERAL_RE.finditer(stripped):
                all_dotted_literals.add(m.group(1))

            for m in DYNAMIC_PREFIX_RE.finditer(stripped):
                # Strip the trailing "." the regex requires before "${" --
                # namespace prefixes are stored dot-free ("nav", not "nav.").
                dynamic_prefixes.add(m.group(1).rstrip("."))

    return all_dotted_literals, dynamic_prefixes


def strip_plural_suffix(key: str) -> str:
    for suffix in PLURAL_SUFFIXES:
        if key.endswith(suffix):
            return key[: -len(suffix)]
    return key


def is_reachable(key: str, all_dotted_literals: set[str], dynamic_prefixes: set[str]) -> bool:
    """True if `key` (a leaf from the English catalogue, e.g.
    "statusBar.downloading_one") is findable in the app by any of the
    three routes this script knows about: its exact text appearing as a
    quoted literal somewhere (whether inside a direct `t('key')` call or
    one step removed, the way `StatusBar.tsx`'s `AFTER_QUEUE_LABEL_KEYS`
    look-up table works), a `t('base', { count })` call whose literal
    text is this key's plural base (`sidebar.updatesAvailable` for
    `sidebar.updatesAvailable_one`), or a dynamic
    `` `namespace.${...}` `` lookup whose namespace this key sits under."""
    if key in all_dotted_literals:
        return True
    base = strip_plural_suffix(key)
    if base != key and base in all_dotted_literals:
        return True
    for prefix in dynamic_prefixes:
        if key == prefix or key.startswith(prefix + "."):
            return True
    return False


# ---------------------------------------------------------------------------
# "How much of the UI is wired up" (#5).
# ---------------------------------------------------------------------------

USE_TRANSLATION_RE = re.compile(r"\buseTranslation\s*\(")


def collect_component_wiring() -> tuple[int, int]:
    """(files calling useTranslation(), total .tsx files), both counted
    under src/components, excluding *.test.tsx -- a test file mounting a
    component tells you nothing about whether that component itself is
    wired for translation."""
    total = 0
    wired = 0
    for f in sorted(COMPONENTS_DIR.rglob("*.tsx")):
        if ".test.tsx" in f.name:
            continue
        total += 1
        try:
            text = f.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        if USE_TRANSLATION_RE.search(strip_line_comments(text)):
            wired += 1
    return wired, total


def check() -> int:
    locale_codes = collect_locale_codes()
    if REFERENCE_LOCALE not in locale_codes:
        # Shouldn't happen -- English is always in LOCALES -- but if the
        # parse fell through to the disk-directory fallback and somehow
        # still missed it, there is nothing to compare against.
        print(f"WARNING: {REFERENCE_LOCALE!r} not found in locale codes {locale_codes}", file=sys.stderr)
        locale_codes = [REFERENCE_LOCALE] + [c for c in locale_codes if c != REFERENCE_LOCALE]

    catalogues: dict[str, dict[str, str]] = {}
    load_errors: list[str] = []
    for code in locale_codes:
        data, err = load_locale(code)
        catalogues[code] = data
        if err:
            load_errors.append(err)

    reference = catalogues.get(REFERENCE_LOCALE, {})
    other_codes = [c for c in locale_codes if c != REFERENCE_LOCALE]

    all_dotted_literals, dynamic_prefixes = collect_used_keys_and_dynamic_prefixes()
    wired, total_components = collect_component_wiring()

    print(f"Languages checked                  : {', '.join(locale_codes)}")
    print(f"Keys in {REFERENCE_LOCALE} (reference)          : {len(reference)}")
    print(f"Quoted dotted-path strings in source: {len(all_dotted_literals)}")
    print(f"Dynamic key namespaces found        : {', '.join(sorted(dynamic_prefixes)) or '(none)'}")
    print()

    findings = 0  # Checks 1-3 only -- #4 and #5 never affect the exit code.

    # ---- Load errors (missing file / invalid JSON) ----------------------
    if load_errors:
        print("### A locale file could not be read\n")
        for msg in load_errors:
            print(f"  • {msg}")
        print()
        findings += len(load_errors)

    # ---- 1. Key parity: a language missing a key English has, or an ----
    # ----    extra key English doesn't have. -----------------------------
    parity_findings: list[str] = []
    for code in other_codes:
        cat = catalogues.get(code, {})
        missing = sorted(set(reference) - set(cat))
        extra = sorted(set(cat) - set(reference))
        path = f"public/locales/{code}/translation.json"
        for key in missing:
            parity_findings.append(f"  • {path} — missing key '{key}' (present in {REFERENCE_LOCALE})")
        for key in extra:
            parity_findings.append(
                f"  • {path} — has key '{key}' that {REFERENCE_LOCALE}/translation.json does not"
            )
    if parity_findings:
        print("### Translation files disagree about which keys exist\n")
        for line in parity_findings:
            print(line)
        print()
        findings += len(parity_findings)

    # ---- 2. Empty translated value --------------------------------------
    empty_findings: list[str] = []
    for code in locale_codes:
        cat = catalogues.get(code, {})
        path = f"public/locales/{code}/translation.json"
        for key in sorted(cat):
            if cat[key] == "":
                empty_findings.append(f"  • {path} — '{key}' is an empty string (renders as nothing)")
    if empty_findings:
        print("### A translated value is an empty string\n")
        for line in empty_findings:
            print(line)
        print()
        findings += len(empty_findings)

    # ---- 3. Placeholder tokens present in English, missing/extra in a ---
    # ----    translation. -------------------------------------------------
    placeholder_findings: list[str] = []
    for key in sorted(reference):
        ref_placeholders = placeholders_in(reference[key])
        for code in other_codes:
            cat = catalogues.get(code, {})
            if key not in cat:
                continue  # already reported by check 1
            cat_placeholders = placeholders_in(cat[key])
            missing_ph = sorted(ref_placeholders - cat_placeholders)
            extra_ph = sorted(cat_placeholders - ref_placeholders)
            path = f"public/locales/{code}/translation.json"
            if missing_ph:
                placeholder_findings.append(
                    f"  • {path} — '{key}' is missing placeholder(s) "
                    f"{', '.join('{{' + p + '}}' for p in missing_ph)} that "
                    f"{REFERENCE_LOCALE}/translation.json has"
                )
            if extra_ph:
                placeholder_findings.append(
                    f"  • {path} — '{key}' has placeholder(s) "
                    f"{', '.join('{{' + p + '}}' for p in extra_ph)} that "
                    f"{REFERENCE_LOCALE}/translation.json does not have"
                )
    if placeholder_findings:
        print("### A {{placeholder}} in English is missing (or added) in a translation\n")
        for line in placeholder_findings:
            print(line)
        print()
        findings += len(placeholder_findings)

    # ---- 4. Unused keys (informational — see the file-level docstring ---
    # ----    section "Why #4 doesn't use bullets" for why these are ------
    # ----    plain lines, not '•' findings, and never affect the exit ----
    # ----    code). --------------------------------------------------------
    unused = sorted(k for k in reference if not is_reachable(k, all_dotted_literals, dynamic_prefixes))
    documented_unused = sorted(k for k in unused if k in UNUSED_KEY_EXCEPTIONS)
    undocumented_unused = sorted(k for k in unused if k not in UNUSED_KEY_EXCEPTIONS)
    stale_exceptions = sorted(
        k for k in UNUSED_KEY_EXCEPTIONS if k in reference and is_reachable(k, all_dotted_literals, dynamic_prefixes)
    )
    missing_exception_keys = sorted(k for k in UNUSED_KEY_EXCEPTIONS if k not in reference)

    print(
        f"### Keys defined in {REFERENCE_LOCALE}/translation.json but not looked up anywhere "
        f"in src/ (informational -- not a fault)\n"
    )
    print(
        f"{len(unused)} of {len(reference)} keys ({len(unused) * 100 // max(len(reference), 1)}%) "
        f"have no `t(...)` call site this script can find -- written ahead of the screen that "
        f"will use them, per this project's own convention. {len(documented_unused)} of those "
        f"are recorded in UNUSED_KEY_EXCEPTIONS as deliberately, permanently unused; the "
        f"remaining {len(undocumented_unused)} are the ones actually waiting on a component."
    )
    if undocumented_unused:
        print()
        for key in undocumented_unused:
            print(f"  - {key}")
    if documented_unused:
        print()
        print("Deliberately unused (see UNUSED_KEY_EXCEPTIONS for why):")
        for key in documented_unused:
            print(f"  - {key}: {UNUSED_KEY_EXCEPTIONS[key]}")
    print()

    if stale_exceptions:
        print("### Recorded in UNUSED_KEY_EXCEPTIONS, but it is no longer true\n")
        for key in stale_exceptions:
            print(
                f"  • tools/audit-checks/check_i18n.py — '{key}' is listed as deliberately unused, "
                f"but a t(...) call site (or dynamic lookup) now reaches it. Remove the entry."
            )
        print()
        findings += len(stale_exceptions)

    if missing_exception_keys:
        print("### Recorded in UNUSED_KEY_EXCEPTIONS, but the key no longer exists\n")
        for key in missing_exception_keys:
            print(
                f"  • tools/audit-checks/check_i18n.py — '{key}' is listed in UNUSED_KEY_EXCEPTIONS "
                f"but is not a key in {REFERENCE_LOCALE}/translation.json any more. Remove the entry."
            )
        print()
        findings += len(missing_exception_keys)

    # ---- 5. Wired-up progress (informational, see docstring) ------------
    pct = wired * 100 // max(total_components, 1)
    print(
        f"### How much of the UI is wired up\n\n"
        f"{wired} of {total_components} .tsx files under src/components ({pct}%) call "
        f"useTranslation()."
    )
    print()

    if (
        not load_errors
        and not parity_findings
        and not empty_findings
        and not placeholder_findings
        and not stale_exceptions
        and not missing_exception_keys
    ):
        print(
            f"OK — {', '.join(locale_codes)} agree on every key, no empty translated values, "
            f"and every {{{{placeholder}}}} in {REFERENCE_LOCALE} is present in every translation."
        )

    if findings and "--strict" in sys.argv:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(check())
