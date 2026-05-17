// Copyright (c) 2026 MeedyaSuite

/**
 * @file Contextual help button that deep-links to a Help page topic.
 *
 * Renders a small "?" icon button inline next to settings labels. On click,
 * it navigates to the Help page and auto-selects the corresponding topic.
 *
 * Uses the `navigateToHelp` action from `uiStore` to set both the current
 * page and the deep-link topic in a single state update.
 *
 * @see {@link @/stores/uiStore.ts}         -- navigateToHelp action
 * @see {@link @/components/help/HelpViewer.tsx} -- consumes helpActiveTopic
 */

import { HelpCircle } from 'lucide-react';
import { useUiStore } from '@/stores/uiStore';

/**
 * Props for the {@link HelpButton} component.
 */
interface HelpButtonProps {
  /** Help topic ID to navigate to (must match a topic.id in HelpViewer) */
  topic: string;
  /** Optional tooltip text shown on hover */
  tooltip?: string;
}

/**
 * Small inline "?" button that navigates to the Help page with a specific
 * topic pre-selected. Designed to sit next to form labels in settings tabs.
 *
 * @example
 * ```tsx
 * <HelpButton topic="cookies-help" tooltip="Learn about cookie authentication" />
 * ```
 */
export function HelpButton({ topic, tooltip }: HelpButtonProps) {
  const navigateToHelp = useUiStore((s) => s.navigateToHelp);

  return (
    <button
      type="button"
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        navigateToHelp(topic);
      }}
      className="
        inline-flex items-center justify-center
        w-4 h-4 rounded-full
        text-content-tertiary
        hover:text-accent hover:bg-accent-light
        transition-colors
        cursor-pointer
        flex-shrink-0
      "
      title={tooltip ?? 'View help'}
      aria-label={tooltip ?? `Help: ${topic}`}
    >
      <HelpCircle size={14} />
    </button>
  );
}
