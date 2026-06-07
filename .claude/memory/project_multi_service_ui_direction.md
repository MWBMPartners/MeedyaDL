---
name: Multi-service UI direction (OnTheSpot-inspired)
description: Design language, anti-patterns, and recommendation rollout for the queue / settings / sidebar polish landing alongside the M8-M10 multi-service expansion. Captured 2026-06-07 via PR audit + codebase verification.
type: project
---

The user agreed on a polish direction inspired by [OnTheSpot](https://github.com/justin025/onthespot)'s clean multi-service queue presentation while explicitly rejecting OnTheSpot's column overload + dated Qt aesthetic. Full proposal lives in **#911** ("Multi-service UI polish: queue + settings + sidebar (OnTheSpot-inspired, codebase-verified)"). The recommendations are codebase-verified — each carries concrete files-to-change + effort tier.

## Rollout phases

- **Phase 1 — pre-M8 polish (Apple-Music-only era benefits)**: responsive column visibility system (#911-0 — foundation, lands first), per-row platform icon (#911-1), album-art thumbnails (#911-2), Artist — Album — Track row (#911-3), status pills (#911-4), hover-reveal actions (#911-5), micro-animations (#911-6), WCAG-AA service brand colour tokens (#911-7), undo for destructive ops (#911-8), per-service download preview card (#911-9).
- **Phase 2 — multi-service-gated (≥ M8)**: service filter chips (#911-10), multi-service empty state (#911-11), unified Settings > Accounts page (#911-12 — defer until ≥ 2 real services), sidebar per-service connection strip (#911-13), first-launch service picker (#911-14).
- **Phase 3 — medium-term**: Cmd/Ctrl+K command palette across Queue+History+Library (#911-15), batch selection + bulk-actions toolbar (#911-16).
- **Phase 4 — defer**: sidebar service grouping (#911-17 — filter chips solve same problem cheaper).

**Recommendation 0 (responsive column system) lands first** — it's the framework all subsequent Phase 1 row work snaps into. Once it's in place, recommendations 1-9 fit comfortably across 2-3 small follow-up PRs in the same release window.

## Responsive column system (the foundation — #911-0)

The queue row uses CSS Grid with up to ~8 columns whose visibility tiers at Tailwind's default breakpoints. Tier 1 is always visible; subsequent tiers reveal at wider widths:

| Tier   | Visibility from   | Columns                                                                            |
| ------ | ----------------- | ---------------------------------------------------------------------------------- |
| Tier 1 | always (≥ 320px)  | Album art · Artist — Album — Track · Status pill · Action buttons (hover-revealed) |
| Tier 2 | `md` (≥ 768px)    | Platform / service icon                                                            |
| Tier 3 | `lg` (≥ 1024px)   | Inline progress bar · Speed / ETA                                                  |
| Tier 4 | `xl` (≥ 1280px)   | Codec / quality · Content type                                                     |
| Tier 5 | `2xl` (≥ 1536px)  | File path · Submitted-at timestamp · Estimated file size                           |

Click-to-expand affordance reveals all hidden columns inline below the row at any width. Sticky column-header strip mirrors the same responsive visibility. Tailwind's defaults (`md` 768, `lg` 1024, `xl` 1280, `2xl` 1536) align with typical Tauri-window-on-desktop widths — no custom `screens` config needed.

## Anti-patterns (load-bearing — do not regress)

These are the "what NOT to do" decisions. Future Claude sessions should consult these before proposing new UI shape:

1. **Use responsive column visibility, never fixed density at all widths.** The queue may surface up to ~8 columns at wide widths, but visibility must tier at Tailwind breakpoints so a sidebar-collapsed narrow window stays scannable. Tier 1 (essentials: art, identifier, status, actions) always visible; subsequent tiers reveal at `md` / `lg` / `xl` / `2xl`. Provide a click-to-expand affordance for narrow-window users. **Don't dump everything onto the row at every width.** (Revised 2026-06-07 — the original "cap at 4-5 columns" rule was too blunt.)
2. **Don't conflate service brand colour with download status.** Status pills (green/amber/red) and brand colours (Apple red, Spotify green, YouTube red, BBC pink, etc.) must use distinct hue ranges so deuteranopia users can tell "Spotify queued" from "Spotify failed."
3. **Don't auto-open service-specific Settings tabs on URL paste.** Hostile mid-flow during a 50-URL batch. Surface the per-service preview card under the textarea instead (#911-9).
4. **Don't gate basic queue functionality behind multi-select mode.** Selection augments per-row actions, never replaces them. A single-row queue should not require selection mode to cancel.

## Why this matters for the prep work happening now

The current `prep/expanded-services-groundwork` branch (see [[project_multi_service_groundwork]]) already added the data foundations: `MediaServiceId` enum, `service` + `engine` fields on `QueueItemStatus`, `serviceStatusStore.ts`, per-service settings stubs. **#911's Phase 1 recommendations layer onto those foundations** — e.g., the service filter chips are a thin UI on top of the existing `service` field; the unified Accounts page reuses `PerServiceSettings` already in settings.rs.

When the M8 BBC iPlayer integration starts landing (#102), the Phase 2 items become unblocked. The natural sequence is: Phase 1 ships during Apple-Music-only era → M8 lands → Phase 2 follows on the same alpha-channel cadence.

## Cross-cutting dependencies

- **#125 a11y EPIC** — recommendation 7's WCAG-AA contrast audit ties directly to #125's broader colour-blindness / high-contrast work. The new `--service-*` tokens must remain visually distinct from `--status-*` tokens (anti-pattern 2).
- **#100 multi-service EPIC** — Phase 2 + 3 items gate on this. Phase 1 does not.
- **#889 Pause/Resume Queue (shipped)** — already provides the non-destructive parallel to the destructive Abort. Recommendation 8 extends the undo affordance from per-row Cancel up to Clear All / Abort All.
- **#620 Abort All Queue (shipped)** — absorbed into the future bulk-actions toolbar (recommendation 16) rather than left as a separate red button.

## Conventions established by #911

- **Platform icon files** live at `public/icons/platforms/<service>.svg` (already in place for 7 services). New services land their icon there in the same shape (inline SVG, `currentColor` for `fill`/`stroke` so theme tokens apply).
- **Service brand colour tokens** are named `--service-<service-kebab-case>` in CSS custom-property files. Tailwind register them under `theme.colors.service.<service>`.
- **Per-row vocabulary** (platform icon → art thumb → Artist — Album — Track → status pill → hover-revealed actions) should be reused everywhere rows appear: Queue, History, Library Scan, and the future Cmd-K results panel. Establishing one row format means users learn it once.
- **"Drop a link" empty states** list only services the user has enabled (via the `enabled_services: Vec<MediaServiceId>` setting introduced in recommendation 14). Pre-M8 this is effectively Apple Music only; post-M8 it grows.

Don't propose UI that violates any of these without explicit user sign-off.

## What's NOT in scope

- Native macOS SwiftUI rewrite (#109) — out of scope for #911; tracked separately.
- Cloud upload UI (#858 / #859 / #860 / #861) — different surface, different EPIC.
- Touch Bar (#386) — macOS-specific, separate.
- Onboarding rewrite beyond recommendation 14's first-launch service picker.
