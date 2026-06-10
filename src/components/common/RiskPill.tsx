// Copyright (c) 2026 MeedyaSuite

/**
 * @file RiskPill — three-tier risk-level visual cue.
 *
 * Renders an icon + label pill next to a toggle / input that
 * communicates the consequence-tier of changing the control's
 * value. Used in the Spotify Settings tab (M9-UI) to make the
 * cost of disabling each anti-ban safeguard legible at a glance,
 * without having to expand a description.
 *
 * ## Why this exists
 *
 * The M9 EPIC ships an account-ban-risk anti-ban stack, surfaced
 * to the user via a consent modal + a Settings tab with knobs.
 * Some knobs are tunings (delay seconds — adjust away); some are
 * safeguard kills (turn off the throttle, set the cap to zero).
 * Without a visual cue, the two look identical and a user could
 * disable the flagship throttle thinking it's just a UI nicety.
 *
 * The pill follows WCAG 1.4.1 — no colour-only signalling. Each
 * tier carries a distinct icon as well as a colour, and an
 * `aria-label` for screen readers.
 *
 * ## Design choices
 *
 * - **Three tiers, not five**. Five tiers (very-low / low / medium /
 *   high / very-high) would force fine distinctions the user can't
 *   act on. Three (lower / higher / highest) maps to the actionable
 *   reality: "fine to change", "think before changing", "don't
 *   change without consent".
 * - **Reusable across services**. Not Spotify-specific. If a future
 *   service (BBC iPlayer — M8, YouTube — M10) needs the same
 *   "consequence-of-changing" cue on a settings knob, this component
 *   handles it.
 *
 * @see SpotifyTab.tsx — primary consumer in M9-UI
 */

import { ShieldCheck, Shield, AlertTriangle } from 'lucide-react';
import type { ReactElement } from 'react';

/**
 * Risk tier for a settings control. Lower = safer to change;
 * highest = changing this materially increases user risk.
 */
export type RiskTier = 'lower' | 'higher' | 'highest';

interface RiskPillProps {
  /** Risk-tier slot the rendered pill represents. */
  tier: RiskTier;
}

interface TierConfig {
  /** Lucide icon component. */
  Icon: ReactElement;
  /** Short text label shown inside the pill. */
  label: string;
  /** Tailwind colour classes for the pill. */
  className: string;
  /** Verbose label read by screen readers (no abbreviations). */
  ariaLabel: string;
}

const TIER_CONFIG: Record<RiskTier, TierConfig> = {
  lower: {
    Icon: <ShieldCheck size={12} aria-hidden="true" />,
    label: 'Lower risk',
    className: 'bg-status-success/10 text-status-success ring-status-success/30',
    ariaLabel: 'Lower risk to change',
  },
  higher: {
    Icon: <Shield size={12} aria-hidden="true" />,
    label: 'Higher risk',
    className: 'bg-status-warning/10 text-status-warning ring-status-warning/30',
    ariaLabel: 'Higher risk to change — pause before adjusting',
  },
  highest: {
    Icon: <AlertTriangle size={12} aria-hidden="true" />,
    label: 'Highest risk',
    className: 'bg-status-error/10 text-status-error ring-status-error/30',
    ariaLabel: 'Highest risk to change — disabling this materially increases account-ban exposure',
  },
};

/**
 * Three-tier risk-level pill. Renders an icon + label in a coloured
 * rounded chip. Reads a verbose tier-specific aria-label so the cue
 * is conveyed to screen readers without relying on the colour or
 * iconography.
 *
 * @example
 *   <Toggle ... />
 *   <RiskPill tier="highest" />
 */
export function RiskPill({ tier }: RiskPillProps) {
  const cfg = TIER_CONFIG[tier];
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-medium ring-1 ${cfg.className}`}
      role="img"
      aria-label={cfg.ariaLabel}
    >
      {cfg.Icon}
      <span aria-hidden="true">{cfg.label}</span>
    </span>
  );
}
