---
name: project-brand-identity
description: MeedyaDL = product name; MeedyaSuite = vendor / publisher / copyright holder. Distinction matters for credits but not for UI strings or paths.
metadata:
  type: project
---

**Vendor vs product naming convention** (locked in 2026-05-17 by the v1.6 pre-PR
sweep). The two words are NOT interchangeable:

- **MeedyaDL** is the product name. Used in: window titles, native installer
  product names (`tauri.conf.json::productName`), the Cargo `[package].name`,
  the npm `name`, the `meedyadl://` URL scheme, the help-doc text body,
  in-app activity log strings, `MeedyaDL-Tools` mirror repo, ARIA labels,
  the brand-kit `MeedyaDL Brand Kit` heading, and every customer-visible
  reference to "the app".
- **MeedyaSuite** is the vendor / publisher / copyright holder. Used in
  copyright notices (`Copyright (c) YYYY MeedyaSuite`), `package.json::author`,
  `Cargo.toml::authors`, the `LICENSE` file, the `tauri.conf.json::copyright`
  bundle field, the `Made with ❤️ by MeedyaSuite` README footer, and the brand
  kit's "All rights reserved" line.

**Why:** MeedyaSuite is the product line; MeedyaDL is one app within it. Before
this convention was established, the codebase had `Copyright (c) 2026 MeedyaDL`
in 291 source-file headers, treating the product as if it were its own vendor.
The pre-PR sweep (`chore(branding): …`, commit `2b62505`) renamed every
vendor/copyright reference to `MeedyaSuite` while leaving every product-name
reference untouched.

**How to apply:** When writing or editing copyright notices, package metadata
(`author`, `authors`, `copyright`), or any line that answers "who made this", use
`MeedyaSuite`. When writing UI text, doc body text, error messages, help text,
ARIA labels, mirror-repo names, or anything that names the app itself, use
`MeedyaDL`. If you're regenerating brand assets (`scripts/generate-icons.mjs`,
`scripts/svg-to-apng.mjs`), the source SVGs already follow this convention —
preserve it.

**Sanity check** — these greps should each return a non-empty result, and the
two sets should NOT overlap on files:

```bash
# vendor refs (should always say MeedyaSuite, never MeedyaDL)
grep -rln "Copyright (c) " --include="*.rs" --include="*.ts" --include="*.tsx" \
    --include="*.toml" --include="*.json" .

# product refs (should always say MeedyaDL, never MeedyaSuite)
grep -rln '"productName"' src-tauri/tauri.conf.json
grep -rln "meedyadl://" src-tauri/
```

The `scripts/update-copyright-year.sh` year-rollover script also encodes this
convention — its `PATTERN_SINGLE` substitution requires the literal word
`MeedyaSuite` after the year, so any future stray `MeedyaDL` vendor reference
will be invisible to the year-bumper and surface as a stale `2026` after the
2027-01-01 run. That's an intentional canary.
