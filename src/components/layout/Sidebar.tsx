// Copyright (c) 2026 MeedyaDL
/**
 * @file Sidebar navigation component.
 *
 * Renders the app's left-side navigation with page links, a dependency
 * readiness indicator, and a collapsible (icon-only) mode. Active page
 * highlighting is driven by `useUiStore.currentPage`.
 *
 * On **macOS**, the sidebar header includes extra top padding (`pt-8`) so
 * that the native traffic-light buttons (close/minimize/maximize) do not
 * overlap the app logo area. This padding is not needed on Windows/Linux
 * where the custom {@link TitleBar} occupies its own row above the sidebar.
 *
 * State connections:
 *  - {@link useUiStore}        -- reads `currentPage` and `sidebarCollapsed`;
 *                                  calls `setPage()` and `toggleSidebar()`.
 *  - {@link useDependencyStore} -- reads `isReady()` to show the green/yellow
 *                                  status dot in the sidebar footer.
 *  - {@link usePlatform}        -- detects macOS for conditional padding.
 *
 * @see https://lucide.dev/icons/       -- icon set used for nav items and controls.
 * @see https://tailwindcss.com/docs/transition-property -- CSS transitions for collapse.
 * @see https://react.dev/reference/react/useState  -- (no local state needed; all in Zustand)
 */

/**
 * Lucide React icons used in the sidebar.
 *
 * Icon-to-page mapping:
 *  - `Download`     -> Download page    (@see https://lucide.dev/icons/download)
 *  - `ListOrdered`  -> Queue page       (@see https://lucide.dev/icons/list-ordered)
 *  - `Clock`        -> History page     (@see https://lucide.dev/icons/clock)
 *  - `Settings`     -> Settings page    (@see https://lucide.dev/icons/settings)
 *  - `HelpCircle`   -> Help page        (@see https://lucide.dev/icons/help-circle)
 *  - `ChevronLeft`  -> Collapse sidebar (@see https://lucide.dev/icons/chevron-left)
 *  - `ChevronRight` -> Expand sidebar   (@see https://lucide.dev/icons/chevron-right)
 */
import {
  Download,
  ListOrdered,
  Clock,
  ScrollText,
  ArrowUpCircle,
  Settings,
  HelpCircle,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
  // Library Scan page (Phase 5 / #717): folder-search icon hints at the
  // "scan an existing on-disk library" UX that the page implements.
  FolderSearch,
} from 'lucide-react';

/**
 * Zustand stores providing reactive UI and dependency state.
 * @see useUiStore in @/stores/uiStore.ts         -- current page & sidebar state
 * @see useDependencyStore in @/stores/dependencyStore.ts -- isReady() for status dot
 */
import { useTranslation } from 'react-i18next';
import { useUiStore } from '@/stores/uiStore';
import { useDependencyStore } from '@/stores/dependencyStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { useUpdateStore } from '@/stores/updateStore';

/** Platform detection hook used to apply macOS-specific spacing. */
import { usePlatform } from '@/hooks/usePlatform';

/** Tooltip component shown when sidebar is collapsed (icon-only mode). */
import { Tooltip } from '@/components/common';

/**
 * `AppPage` union type: 'download' | 'queue' | 'settings' | 'help'.
 * Used to type-check page identifiers in navigation items.
 * @see AppPage in @/types/index.ts
 */
import type { AppPage } from '@/types';

/**
 * Shape of a single navigation item in the sidebar.
 *
 * Each item maps an {@link AppPage} identifier to a display label and
 * a Lucide icon component. The `icon` field is typed as `typeof Download`
 * (i.e., a Lucide icon component constructor) so that any Lucide icon
 * can be assigned while remaining type-safe.
 */
interface NavItem {
  /** Page identifier that corresponds to one of the `AppPage` union members. */
  page: AppPage;
  /** Human-readable label displayed next to the icon (hidden when collapsed). */
  label: string;
  /**
   * Lucide icon component rendered at 18px.
   * Typed as `typeof Download` -- a Lucide React component constructor.
   * @see https://lucide.dev/guide/packages/lucide-react
   */
  icon: typeof Download;
  /** Optional keyboard shortcut hint (displayed in tooltip). */
  shortcut?: string;
}

/**
 * Ordered array of navigation items rendered in the sidebar.
 *
 * The order here determines the visual order in the UI. Each entry
 * connects a route identifier (`page`) to its icon and label. When the
 * user clicks a nav button, `useUiStore.setPage(page)` is called,
 * which triggers the App-level page router to render the corresponding
 * page component.
 *
 * To add a new page to the sidebar:
 *  1. Add a new member to the `AppPage` union type in `@/types/index.ts`.
 *  2. Add an entry to this array with the matching `page` identifier.
 *  3. Handle the new page in the App-level page switch/router.
 */
const NAV_ITEMS: NavItem[] = [
  { page: 'download', label: 'Download', icon: Download, shortcut: 'D' },
  { page: 'queue', label: 'Queue', icon: ListOrdered, shortcut: 'Q' },
  // Library Scan (Phase 5 / #717) — placed between Queue and History so
  // the user-flow ordering reads "queue work → scan library → see history".
  { page: 'library', label: 'Library', icon: FolderSearch },
  { page: 'history', label: 'History', icon: Clock },
  { page: 'activity', label: 'Activity', icon: ScrollText },
  { page: 'updates', label: 'Updates', icon: ArrowUpCircle },
  { page: 'settings', label: 'Settings', icon: Settings, shortcut: ',' },
  { page: 'help', label: 'Help', icon: HelpCircle },
];

/**
 * Renders the sidebar navigation panel with page links and a status indicator.
 *
 * The sidebar has two visual modes controlled by `useUiStore.sidebarCollapsed`:
 *  - **Expanded** (`w-56` / 224px): icon + label for each nav item, full app
 *    title, and text status indicator.
 *  - **Collapsed** (`w-16` / 64px): icon-only buttons wrapped in right-aligned
 *    {@link Tooltip} components so users can still identify each page.
 *
 * The transition between modes is animated via `transition-all duration-200`
 * (200ms ease) on the `<aside>` element.
 *
 * Three logical sections stack vertically:
 *  1. **Header** -- App logo + title (drag-region for window dragging).
 *  2. **Navigation** -- Page link buttons mapped from {@link NAV_ITEMS}.
 *  3. **Footer** -- Dependency status dot + collapse/expand toggle.
 *
 * @see https://tailwindcss.com/docs/width         -- w-16 / w-56 width classes
 * @see https://tailwindcss.com/docs/transition-property -- transition-all animation
 * @see https://react.dev/reference/react/Fragment  -- used implicitly via JSX
 */
export function Sidebar() {
  // ---------------------------------------------------------------
  // Store selectors (Zustand)
  // ---------------------------------------------------------------
  // Each selector subscribes to a single slice of state to minimize
  // unnecessary re-renders. Zustand uses shallow equality by default.
  // @see https://docs.pmnd.rs/zustand/guides/prevent-rerenders-with-use-shallow

  /** The currently active page identifier (e.g., 'download', 'queue'). */
  const currentPage = useUiStore((s) => s.currentPage);
  /** Navigates to a different page by updating `currentPage` in uiStore. */
  const setPage = useUiStore((s) => s.setPage);
  /** Whether the sidebar is in collapsed (icon-only) mode. */
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  /** Toggles the sidebar between expanded and collapsed modes. */
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  /**
   * Colour-blind mode setting — determines which SVG colour mode to use
   * for the sidebar logo and logotype. Maps to ?mode= URL parameter.
   * See: https://github.com/MWBMPartners/MeedyaDL/issues/220
   */
  const colourBlindMode = useSettingsStore((s) => s.settings.colour_blind_mode);
  /** Build the SVG mode query string for colour-blind aware rendering */
  const svgMode = colourBlindMode ? `?mode=cb-${colourBlindMode}` : '';
  /**
   * Dependency status for Python and GAMDL -- the two required dependencies.
   * We subscribe to the individual status objects (not the `isReady()` getter
   * function) so that the Sidebar re-renders when dependency status actually
   * changes. The previous pattern `(s) => s.isReady` subscribed to the
   * function reference (which is stable/constant), meaning the status dot
   * would never update reactively.
   * @see useDependencyStore in @/stores/dependencyStore.ts
   */
  const python = useDependencyStore((s) => s.python);
  const gamdl = useDependencyStore((s) => s.gamdl);
  /** Derived readiness check: both Python and GAMDL must be installed. */
  const isReady = !!(python?.installed && gamdl?.installed);

  /** Whether the update checker is currently running. */
  const isChecking = useUpdateStore((s) => s.isChecking);
  /** Trigger an update check against PyPI + GitHub Releases. */
  const checkForUpdates = useUpdateStore((s) => s.checkForUpdates);
  /** Derived: true when at least one non-dismissed, compatible update exists. */
  const hasUpdates = useUpdateStore((s) => s.hasActiveUpdates());
  /** Count of actionable updates for the badge label. */
  const updateCount = useUpdateStore((s) => s.getActiveUpdates().length);

  /**
   * Platform detection -- `isMacOS` is used to add extra top padding in the
   * header area so the macOS traffic-light buttons do not overlap the logo.
   */
  const { isMacOS } = usePlatform();

  /** i18n translation function for nav labels and status text. */
  const { t } = useTranslation();

  /** Map page IDs to translated nav labels (fallback to static label). */
  const navLabel = (item: NavItem): string => {
    const key = `nav.${item.page}`;
    const translated = t(key);
    // If i18n returns the key itself (missing translation), use the static label
    return translated === key ? item.label : translated;
  };

  return (
    /**
     * Root `<aside>` element for the sidebar.
     *
     * `flex flex-col` -- vertical column layout for header / nav / footer.
     * `bg-sidebar-bg` and `border-r border-sidebar-border` -- themed
     * background and right-edge border from the app's design tokens.
     * `transition-all duration-200` -- smoothly animates width changes.
     * Width toggles between `w-16` (64px, collapsed) and `w-56` (224px, expanded).
     *
     * @see https://tailwindcss.com/docs/transition-property
     */
    <aside
      className={`
        flex flex-col bg-sidebar-bg border-r border-sidebar-border
        transition-all duration-200
        ${sidebarCollapsed ? 'w-16' : 'w-56'}
      `}
    >
      {/*
       * ---------------------------------------------------------------
       * Section 1: App title / logo area
       * ---------------------------------------------------------------
       * `drag-region` makes this area a window drag handle (Tauri CSS).
       * On macOS, `pt-8` adds 32px top padding to avoid overlapping
       * the native traffic-light window buttons that sit at the top-left
       * of the webview when `titleBarStyle: 'overlay'` is active.
       * On Windows/Linux the TitleBar component occupies its own row
       * above the sidebar, so standard `py-3` padding is sufficient.
       */}
      <div
        className={`
          drag-region px-4 border-b border-sidebar-border flex items-center
          ${isMacOS ? 'pt-8 pb-3' : 'py-3'}
          ${sidebarCollapsed ? 'justify-center' : 'gap-3'}
        `}
      >
        {/*
         * Animated brand logo (SVG with vinyl/reel crossfade).
         * Uses <object> for full SVG animation support; the SVG auto-detects
         * dark mode via @media(prefers-color-scheme) and colour-blind modes
         * via inherited CSS classes. Falls back to static PNG via <img>.
         * `no-drag` ensures the logo isn't treated as a drag handle.
         */}
        {/* SVG loaded as <img> prevents script execution (CSP defence-in-depth).
         * CSS animations still work; only embedded <script> is blocked.
         * See: https://github.com/MWBMPartners/MeedyaDL/issues/221 */}
        <img
          src={`/logo.svg${svgMode}`}
          alt="MeedyaDL logo"
          className="w-16 h-16 rounded-platform flex-shrink-0 no-drag"
        />
        {/*
         * Animated brand logotype (SVG with gradient shimmer).
         * Hidden when the sidebar is collapsed. Falls back to text + static PNG.
         * `no-drag` exempts this from the drag-region so that future
         * interactive elements placed here (e.g., a dropdown) will work.
         */}
        {!sidebarCollapsed && (
          <>
            <img
              src={`/logotype.svg${svgMode}`}
              alt="MeedyaDL"
              className="h-16 flex-1 min-w-0 no-drag"
              onError={(e) => {
                // Fallback to text if SVG fails to load
                const target = e.currentTarget;
                target.style.display = 'none';
                const fallback = target.nextElementSibling as HTMLElement;
                if (fallback) fallback.style.display = 'block';
              }}
            />
            <div className="no-drag hidden">
              <h1 className="text-sm font-semibold text-sidebar-text-active leading-tight">
                MeedyaDL
              </h1>
              <p className="text-[11px] text-content-secondary leading-tight">Media Downloader</p>
            </div>
          </>
        )}
      </div>

      {/*
       * ---------------------------------------------------------------
       * Section 2: Navigation links
       * ---------------------------------------------------------------
       * Maps over the static `NAV_ITEMS` array to render one button per
       * page. The active page is highlighted with `bg-sidebar-active`
       * and `font-medium`. Inactive items show a hover background.
       *
       * When collapsed, each button is wrapped in a right-aligned
       * Tooltip so the user can still identify the page from its label.
       *
       * `space-y-1` adds 4px vertical gap between nav buttons.
       */}
      <nav className="flex-1 p-2 space-y-1" aria-label="Main navigation">
        {NAV_ITEMS.map((navItem) => {
          const { page, icon: Icon } = navItem;
          const label = navLabel(navItem);
          /** Whether this nav item corresponds to the currently active page. */
          const isActive = currentPage === page;

          /**
           * The nav button element -- built separately so it can optionally
           * be wrapped in a `<Tooltip>` when the sidebar is collapsed.
           *
           * `no-drag` prevents the button from acting as a drag handle.
           * `rounded-platform` applies the platform-appropriate border radius.
           * Active state: `bg-sidebar-active text-sidebar-text-active font-medium`.
           * Inactive state: `text-sidebar-text hover:bg-sidebar-hover`.
           */
          // Build tooltip with optional shortcut hint
          const shortcutHint = navItem.shortcut
            ? ` (${isMacOS ? '\u2318' : 'Ctrl+'}${navItem.shortcut})`
            : '';

          const button = (
            <button
              key={page}
              onClick={() => setPage(page)}
              title={`${label}${shortcutHint}`}
              aria-label={label}
              aria-current={isActive ? 'page' : undefined}
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
              {/* Lucide icon at 18px; flex-shrink-0 prevents icon squishing */}
              <Icon size={18} className="flex-shrink-0" />
              {/* Label text is hidden when the sidebar is collapsed */}
              {!sidebarCollapsed && <span>{label}</span>}
            </button>
          );

          /*
           * When collapsed, wrap the button in a Tooltip positioned to the
           * right of the sidebar so the label is visible on hover.
           * @see Tooltip component in @/components/common
           */
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

      {/*
       * ---------------------------------------------------------------
       * Section 3: Footer -- status indicator + collapse toggle
       * ---------------------------------------------------------------
       * A thin separator (`border-t`) visually divides the footer from
       * the navigation section above it.
       */}
      <div className="p-3 border-t border-sidebar-border">
        {/*
         * Dependency readiness indicator.
         *
         * Shows a small colored dot:
         *  - Green (`bg-status-success`) when `isReady()` returns true
         *    (Python and GAMDL are both installed).
         *  - Yellow (`bg-status-warning`) when setup is incomplete.
         *
         * In expanded mode, the dot is followed by a text label
         * ("Ready" or "Setup Required"). In collapsed mode, the dot
         * is wrapped in a right-aligned Tooltip.
         *
         * `isReady` is a derived boolean computed from the `python` and
         * `gamdl` status objects subscribed to above.
         * @see useDependencyStore in @/stores/dependencyStore.ts
         */}
        {!sidebarCollapsed ? (
          <div className="flex items-center gap-2 text-xs text-content-tertiary">
            <span
              className={`w-2 h-2 rounded-full flex-shrink-0 ${
                isReady ? 'bg-status-success' : 'bg-status-warning'
              }`}
            />
            {isReady ? t('sidebar.ready') : t('sidebar.setupRequired')}
          </div>
        ) : (
          <Tooltip content={isReady ? t('sidebar.ready') : t('sidebar.setupRequired')} position="right">
            <div className="flex justify-center">
              <span
                className={`w-2 h-2 rounded-full ${
                  isReady ? 'bg-status-success' : 'bg-status-warning'
                }`}
              />
            </div>
          </Tooltip>
        )}

        {/*
         * Check for Updates button.
         *
         * Triggers `checkForUpdates()` from updateStore. When updates are
         * available, navigates to Settings so the user can review and apply them.
         *
         * Visual states:
         *  - Default: RefreshCw icon + "Check for Updates"
         *  - Checking: spinning icon + "Checking..."
         *  - Updates found: accent-coloured badge + "N Update(s)"
         *
         * In collapsed mode, the button shows just the icon wrapped in a Tooltip.
         * When updates are available, a small accent dot overlays the icon.
         */}
        {(() => {
          const handleUpdateClick = () => {
            if (hasUpdates) {
              setPage('updates');
            } else if (!isChecking) {
              checkForUpdates().catch(() => {});
            }
          };

          const label = isChecking
            ? t('sidebar.checking')
            : hasUpdates
              ? t('sidebar.updatesAvailable', { count: updateCount })
              : t('sidebar.checkForUpdates');

          const updateButton = (
            <button
              type="button"
              onClick={handleUpdateClick}
              disabled={isChecking}
              className={`
                no-drag w-full flex items-center gap-2 mt-2 px-2 py-1.5
                rounded-platform text-xs transition-colors
                ${sidebarCollapsed ? 'justify-center' : ''}
                ${
                  hasUpdates
                    ? 'text-accent hover:bg-sidebar-hover'
                    : 'text-content-tertiary hover:text-content-primary hover:bg-sidebar-hover'
                }
              `}
              aria-label={label}
            >
              <span className="relative flex-shrink-0">
                <RefreshCw size={14} className={isChecking ? 'animate-spin' : ''} />
                {hasUpdates && (
                  <span className="absolute -top-1 -right-1 w-2 h-2 rounded-full bg-accent" />
                )}
              </span>
              {!sidebarCollapsed && <span>{label}</span>}
            </button>
          );

          if (sidebarCollapsed) {
            return (
              <Tooltip content={label} position="right">
                {updateButton}
              </Tooltip>
            );
          }

          return updateButton;
        })()}

        {/*
         * Collapse / expand toggle button.
         *
         * Clicking this calls `useUiStore.toggleSidebar()` which flips
         * `sidebarCollapsed` and triggers the width transition on `<aside>`.
         *
         * Icon switches between:
         *  - ChevronRight (collapsed -> "click to expand")
         *  - ChevronLeft  (expanded  -> "click to collapse")
         *
         * `no-drag` prevents the button from acting as a window drag handle.
         * `aria-label` provides an accessible description of the action.
         */}
        <button
          onClick={toggleSidebar}
          className="no-drag w-full flex items-center justify-center mt-2 p-1.5 rounded-platform text-content-tertiary hover:text-content-primary hover:bg-sidebar-hover transition-colors"
          aria-label={sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        >
          {sidebarCollapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
        </button>
      </div>
    </aside>
  );
}
