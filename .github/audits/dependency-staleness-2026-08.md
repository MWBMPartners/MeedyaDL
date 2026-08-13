# Dependency Staleness Audit — August 2026

**Date:** 2026-08-13
**Trigger:** Follow-up to CVE-2026-56876 (extract-zip symlink path traversal, GHSA-jmr9-qjv8-65gv). extract-zip was an *unmaintained* transitive dependency with **no patched version** — the archetype we wanted to proactively hunt for elsewhere in the tree.
**Method:** Automated last-release scan of the full transitive tree → judgement-based triage (function × maintenance × reachability) → per-candidate deep-dive → adversarial completeness critic.

---

## TL;DR

**No "next extract-zip" exists in the tree.** Of 1,336 packages, 242 are ≥2 years stale (or deprecated/yanked), but every one of those resolves to *stable-and-safe* or *maintained-enough*. **Zero packages require urgent action.** The single item worth passively monitoring is `lzma-sys` (bundled C liblzma on the archive-extraction path — well-mitigated). A few non-security hygiene tidy-ups are optional.

| Posture | Count | Meaning |
| --- | --- | --- |
| **act-now** | **0** | Nothing needs an urgent fix |
| **plan-migration** | **0** | Nothing needs a scheduled swap |
| **monitor** | **1** | `lzma-sys` — keep an eye on it |
| **watch** (context only) | ~16 | Security-adjacent but maintained/insulated |
| **ignore** (stable-by-design) | **225** | Frozen micro-libs; staleness is benign |

---

## Scope & method

- **Trees scanned:** 539 npm packages (`package-lock.json`) + 797 Rust crates (`src-tauri/Cargo.lock`) = **1,336** total, full transitive depth.
- **Flag heuristic:** last registry publish ≥ 2 years ago, **or** an explicit `deprecated`/`yanked` marker → **242 candidates** (115 npm, 127 cargo).
- **Why age alone is nearly useless:** the vast majority of stale packages are tiny "done" libraries (`is-decimal`, `fnv`, `winapi-*` shims, `natural-compare`) that need no changes and carry no security surface. Real risk = **security-sensitive function** AND **genuine abandonment** AND **reachable in MeedyaDL** (ideally with untrusted input). Only that intersection matters.
- **Triage + deep-dive + adversarial critic:** 6 triage agents classified all 242; 12 candidates got individual deep-dives (upstream archived-status, CVE/RUSTSEC history, maintained alternative, MeedyaDL reachability); an adversarial completeness critic independently re-read all 242 to catch any wrongly-dismissed package and to sanity-check every severity. **The critic's verdict: no miss — the exclusion logic is sound.**

---

## The one to monitor

### `lzma-sys` (Rust, `monitor`)
- **What:** C-FFI bindings to bundled `liblzma` (xz) — decompresses archive data.
- **Reachability:** ships in the binary via `zip 2.4.2 → xz2 0.1.7 → lzma-sys 0.1.20` — the tool-archive extraction path (`dependency_manager` unpacks downloaded FFmpeg/MP4Box/etc. bundles). This is the closest thing in the tree to the extract-zip archetype: C decompressor of network-delivered archives.
- **Why only monitor, not act:** no abandonment signal (maintained by Alex Crichton; repo not archived), **no CVE/RUSTSEC** for this crate/version, the bundled liblzma is the pre-backdoor **5.2.x** line (CVE-2024-3094 hit 5.6.x only), the XZ code path is effectively **dormant** (the tool zips use deflate, not xz), and MeedyaDL's extractor (`utils/archive.rs`) already guards zip-slip via `enclosed_name()` and does **not** materialise symlink entries.
- **Action:** none required. Re-check when the next `zip`-crate bump lands, or migrate to the maintained `liblzma` successor crate if `xz2`/`lzma-sys` ever go fully dormant.

---

## Watch list (context only — no action required)

These are security-*adjacent* and shipped/relevant, but each fails at least one leg of the risk triad (maintained, or insulated, or only handles trusted input). Listed so the next audit has continuity.

| Package | Eco | Function | Why it's fine |
| --- | --- | --- | --- |
| `untrusted` | cargo | Zero-alloc parser for crypto/cert input (under `ring`/`rustls`) | Correct-by-design "done" primitive; only historic advisory fixed in 0.6.2; effectively maintained |
| `tokio-native-tls` | cargo | Async TLS adapter over native-tls | Thin wrapper; delegates all crypto to OS TLS; hyperium/tokio org; zero advisory history |
| `hyper-tls` | cargo | TLS support for hyper client | Maintained (hyperium), thin native-tls wrapper, no advisories |
| `tiny-keccak` | cargo | SHA-3 / Keccak hashing | **Build-time only** (via `const-random` proc-macro); frozen FIPS-202 standard; no advisories ever |
| `cesu8` | cargo | CESU-8 / Java modified-UTF-8 decoder | **Target-gated out** — pulled only via `jni` (Android), not compiled on desktop; decodes trusted JVM strings |
| `ntfs` | cargo | `no_std` binary NTFS parser | `#![forbid(unsafe_code)]`; Windows-only cookie-import; input is the **user's own** mounted volume; zero advisories |
| `lzma-rs` | cargo | Pure-Rust LZMA/XZ decoder | Pure-Rust ⇒ DoS-only ceiling; no path handling of its own; maintained "done" decoder |
| `xz2` | cargo | liblzma binding (via `lzma-sys`) | Insulated behind the maintained `zip` crate; same posture as `lzma-sys` |
| `fs2` | cargo | Filesystem free-space + advisory locking | Direct dep, 7.5 yr stale, but only a **metadata syscall** on a trusted path; no advisories |
| `rawcopy-rs-next` | cargo | Windows raw-NTFS reader (heavy-unsafe FFI) | Windows-only cookie import; reads the **user's own** boot volume; the residual-unsafe carrier above `ntfs` — keep explicitly on the radar |
| `rehype-sanitize` | npm | HTML/XSS sanitiser for rendered markdown | The actual XSS boundary for Help docs — **on latest, maintained** (wooorm); load-bearing and covered |
| `rehype-raw` | npm | Reparses raw HTML in markdown → AST | Maintained; invariant holds (`rehype-sanitize` runs **after** it in `HelpViewer.tsx`) |
| `micromark-extension-gfm-tagfilter` | npm | GFM dangerous-tag filter | Part of the maintained wooorm markdown pipeline |
| `pngjs` | npm | Pure-JS PNG codec | **Dev-only** (icon/APNG build scripts); never ships; not deprecated |
| `saxes` | npm | Streaming XML parser | **Dev-only** (jsdom test env); never ships; zero advisories (upstream archived Dec-2025 but irrelevant when it never ships) |
| `git-raw-commits` | npm | Parses local `git log` output | **DEPRECATED** but dev-only changelog tooling; parses trusted local git output (see hygiene below) |

---

## Optional hygiene (non-security)

1. **`@testing-library/dom` is mis-declared as a runtime `dependency`** in `package.json` (it belongs under `devDependencies`). It's test-only (pulls `lz-string` transitively) and is tree-shaken out of the Vite prod bundle, so it doesn't actually ship — but moving it prevents a test library from ever being treated as runtime. **Actionable — fixed separately.**
2. **`git-raw-commits` is deprecated** ("use `@conventional-changelog/git-client` instead"). **Not directly actionable** — it is a *transitive* dep (`@commitlint/cli → @commitlint/read → git-raw-commits@5.0.1`), not something MeedyaDL declares. It resolves on its own when `@commitlint` migrates upstream; there is no clean override (different package name/API). No action for us.
3. **`fs2` → `fs4`: not recommended.** `fs2` is a 7.5-year-stale **direct** dependency, but on inspection `fs4` 1.x is **not** a clean drop-in — it reorganized into a feature-gated, backend-split API (`sync`/`tokio`/`async-std`/`smol`) and no longer exposes `available_space` as a crate-root free function. MeedyaDL uses `fs2` at exactly **one** callsite (`health_check_service.rs::available_space` on a trusted, user-chosen path) with **zero advisory history**. Migrating a shipped-binary code path for no functional or security gain is a poor trade. **Leave `fs2` as-is**; revisit only if it ever acquires an advisory.

---

## What was explicitly ruled out (exclusion evidence)

The adversarial critic verified these security-adjacent-but-safe dismissals so future audits don't re-litigate them:

- **Phantom / not compiled:** the `actix-*` / `language-tags` / `serde_urlencoded`(actix path) crates are pulled only by `sentry-actix`, an **optional** sentry feature MeedyaDL does **not** enable (`sentry` features = `["tracing"]`) — no inbound HTTP server is compiled.
- **Target-gated out:** `cesu8`, `combine` (via `jni`) compile for Android/JVM interop only, not MeedyaDL's desktop targets.
- **Genuinely maintained** (correctly *not* flagged, positive controls): `libesedb-sys 0.2.1` (raw C ESE cookie-DB parser — published 2025-06) and `lz4_flex 0.14.0` (published 2026-07) are the two highest-sensitivity runtime parsers on the cookie-import feature and are both current.
- **Trusted-input FFI:** `minimal-lexical`/`nom` → `libesedb-sys` build-time C-source patching (trusted patch files); `symlink` → `tracing-appender` (creates its own trusted log symlink); `privilege` → a privilege-level *check*, not a parser.
- **225 stable micro-libs:** the bulk of the 242 — frozen "done" utilities with negligible security surface.

---

## Caveats

- **Two deep-dives hit the structured-output retry cap** (`git-raw-commits`, `xz2`) and returned no formal record — but both are independently covered by triage and by the adversarial critic (which explicitly re-verified `xz2` as correctly tiered and `git-raw-commits` as deprecated-but-benign).
- **5 crates were absent from crates.io** (first-party/workspace members) and are out of scope for staleness.
- Staleness age is a *coarse* signal; "last release" can equally mean "abandoned" or "finished." Every flagged package's true status was judged individually, not by age.

---

## Recommended cadence

- **Re-run this audit quarterly** (or after a major framework bump). The scan scripts are trivial to reproduce: enumerate `package-lock.json` / `Cargo.lock`, query `npm view <pkg> time.modified` and `crates.io/api/v1/crates/<name>`, flag ≥2 yr, then triage.
- **Version currency is already automated** by the Dependabot 7-day cooldown + auto-merge policy (#1087). This audit catches the *complementary* class Dependabot cannot: the transitive **unmaintained leaf** with no upgrade to offer — exactly how extract-zip slipped through.
- **Next-audit carry-forward:** re-check `lzma-sys`/`xz2` and `rawcopy-rs-next` first.
