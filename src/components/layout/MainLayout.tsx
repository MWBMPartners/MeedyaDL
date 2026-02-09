/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Main application layout component.
 * Assembles the full app shell: sidebar navigation, custom title bar
 * (Windows/Linux), main content area with page routing, and status bar.
 * The content area renders the appropriate page based on the UI store's
 * currentPage value.
 */

import type { ReactNode } from 'react';
import { Sidebar } from './Sidebar';
import { TitleBar } from './TitleBar';
import { StatusBar } from './StatusBar';
import { ToastContainer } from '@/components/common';

interface MainLayoutProps {
  /** The page content to render in the main area */
  children: ReactNode;
}

/**
 * Renders the full application shell layout:
 * - Custom title bar (Windows/Linux only)
 * - Sidebar navigation (left)
 * - Main content area (center, scrollable)
 * - Status bar (bottom)
 * - Toast notification overlay
 *
 * @param children - The active page component to render in the content area
 */
export function MainLayout({ children }: MainLayoutProps) {
  return (
    <div className="flex flex-col h-screen">
      {/* Custom title bar for Windows/Linux (hidden on macOS) */}
      <TitleBar />

      {/* Main body: sidebar + content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left sidebar navigation */}
        <Sidebar />

        {/* Right content area with status bar */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Page content (scrollable) */}
          <main className="flex-1 overflow-y-auto">{children}</main>

          {/* Bottom status bar */}
          <StatusBar />
        </div>
      </div>

      {/* Toast notification overlay (renders in top-right corner) */}
      <ToastContainer />
    </div>
  );
}
