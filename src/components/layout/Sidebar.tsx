/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Sidebar navigation component.
 * Renders the app's left-side navigation with page links, status indicator,
 * and collapsible behavior. Active page highlighting is driven by the UI store.
 * On macOS, the sidebar includes extra top padding for the traffic-light buttons.
 */

import {
  Download,
  ListOrdered,
  Settings,
  HelpCircle,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react';
import { useUiStore } from '@/stores/uiStore';
import { useDependencyStore } from '@/stores/dependencyStore';
import { usePlatform } from '@/hooks/usePlatform';
import { Tooltip } from '@/components/common';
import type { AppPage } from '@/types';

/** Navigation item definition */
interface NavItem {
  /** Page identifier */
  page: AppPage;
  /** Display label */
  label: string;
  /** Lucide icon component */
  icon: typeof Download;
}

/** Ordered list of navigation items */
const NAV_ITEMS: NavItem[] = [
  { page: 'download', label: 'Download', icon: Download },
  { page: 'queue', label: 'Queue', icon: ListOrdered },
  { page: 'settings', label: 'Settings', icon: Settings },
  { page: 'help', label: 'Help', icon: HelpCircle },
];

/**
 * Renders the sidebar navigation panel with page links and a status indicator.
 * Supports collapsing to icon-only mode for more content space.
 */
export function Sidebar() {
  const currentPage = useUiStore((s) => s.currentPage);
  const setPage = useUiStore((s) => s.setPage);
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  const isReady = useDependencyStore((s) => s.isReady);
  const { isMacOS } = usePlatform();

  return (
    <aside
      className={`
        flex flex-col bg-sidebar-bg border-r border-sidebar-border
        transition-all duration-200
        ${sidebarCollapsed ? 'w-16' : 'w-56'}
      `}
    >
      {/* App title / logo area */}
      {/* Extra top padding on macOS for the native traffic-light window buttons */}
      <div
        className={`
          drag-region px-4 border-b border-sidebar-border flex items-center
          ${isMacOS ? 'pt-8 pb-3' : 'py-3'}
          ${sidebarCollapsed ? 'justify-center' : 'gap-3'}
        `}
      >
        {/* App icon placeholder (could be replaced with SVG icon) */}
        <div className="w-8 h-8 rounded-platform bg-accent flex items-center justify-center flex-shrink-0">
          <Download size={16} className="text-content-inverse" />
        </div>
        {!sidebarCollapsed && (
          <div className="no-drag">
            <h1 className="text-sm font-semibold text-sidebar-text-active leading-tight">
              GAMDL
            </h1>
            <p className="text-[11px] text-content-secondary leading-tight">
              Apple Music Downloader
            </p>
          </div>
        )}
      </div>

      {/* Navigation links */}
      <nav className="flex-1 p-2 space-y-1">
        {NAV_ITEMS.map(({ page, label, icon: Icon }) => {
          const isActive = currentPage === page;
          const button = (
            <button
              key={page}
              onClick={() => setPage(page)}
              className={`
                no-drag w-full flex items-center gap-3 px-3 py-2
                rounded-platform text-sm transition-colors
                ${sidebarCollapsed ? 'justify-center' : ''}
                ${
                  isActive
                    ? 'bg-sidebar-active text-sidebar-text-active font-medium'
                    : 'text-sidebar-text hover:bg-sidebar-hover'
                }
              `}
            >
              <Icon size={18} className="flex-shrink-0" />
              {!sidebarCollapsed && <span>{label}</span>}
            </button>
          );

          /* Wrap in tooltip when collapsed for icon-only mode */
          if (sidebarCollapsed) {
            return (
              <Tooltip key={page} content={label} position="right">
                {button}
              </Tooltip>
            );
          }

          return button;
        })}
      </nav>

      {/* Status indicator at the bottom */}
      <div className="p-3 border-t border-sidebar-border">
        {/* Dependency readiness indicator */}
        {!sidebarCollapsed ? (
          <div className="flex items-center gap-2 text-xs text-content-tertiary">
            <span
              className={`w-2 h-2 rounded-full flex-shrink-0 ${
                isReady() ? 'bg-status-success' : 'bg-status-warning'
              }`}
            />
            {isReady() ? 'Ready' : 'Setup Required'}
          </div>
        ) : (
          <Tooltip
            content={isReady() ? 'Ready' : 'Setup Required'}
            position="right"
          >
            <div className="flex justify-center">
              <span
                className={`w-2 h-2 rounded-full ${
                  isReady() ? 'bg-status-success' : 'bg-status-warning'
                }`}
              />
            </div>
          </Tooltip>
        )}

        {/* Collapse/expand toggle button */}
        <button
          onClick={toggleSidebar}
          className="no-drag w-full flex items-center justify-center mt-2 p-1.5 rounded-platform text-content-tertiary hover:text-content-primary hover:bg-sidebar-hover transition-colors"
          aria-label={sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        >
          {sidebarCollapsed ? (
            <ChevronRight size={16} />
          ) : (
            <ChevronLeft size={16} />
          )}
        </button>
      </div>
    </aside>
  );
}
