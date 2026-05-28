/* eslint-disable @typescript-eslint/ban-ts-comment */
// @ts-nocheck — Staged for future multi-service integration (M8-M10).
// Imports like getServiceLabel don't exist yet. Remove when wiring in.
//
// Copyright (c) 2026 MeedyaDL

/**
 * @file ServiceStatusBanner -- Displays warnings when services are remotely
 * disabled via the kill-switch system, or shows global announcements.
 *
 * Positioned below the UpdateBanner in MainLayout. Banners are non-dismissible
 * while the service remains disabled (they reflect live remote state).
 *
 * @see src/stores/serviceStatusStore.ts -- Zustand store backing this component
 * @see src-tauri/src/services/service_status.rs -- Rust service
 */

import { AlertTriangle, Info } from 'lucide-react';
import { useServiceStatusStore } from '@/stores/serviceStatusStore';
import { getServiceLabel } from '@/lib/url-parser';
import type { MediaServiceId } from '@/types';

/**
 * ServiceStatusBanner -- Renders warning banners for disabled services
 * and info banners for global announcements.
 *
 * Reads from the serviceStatusStore. Returns null when all services are
 * enabled and no global message exists.
 */
export function ServiceStatusBanner() {
  const disabledServices = useServiceStatusStore((s) => s.getDisabledServices)();
  const getServiceMessage = useServiceStatusStore((s) => s.getServiceMessage);
  const globalMessage = useServiceStatusStore((s) => s.getGlobalMessage)();

  if (disabledServices.length === 0 && !globalMessage) {
    return null;
  }

  return (
    <div className="space-y-2 px-4 pt-2">
      {/* Global announcement banner */}
      {globalMessage && (
        <div className="flex items-start gap-2.5 rounded-platform border border-accent/30 bg-accent/5 px-3 py-2.5">
          <Info size={16} className="text-accent flex-shrink-0 mt-0.5" />
          <p className="text-xs text-content-primary">{globalMessage}</p>
        </div>
      )}

      {/* Per-service disabled banners */}
      {disabledServices.map((serviceId: MediaServiceId) => {
        const message = getServiceMessage(serviceId);
        const label = getServiceLabel(serviceId);
        return (
          <div
            key={serviceId}
            className="flex items-start gap-2.5 rounded-platform border border-status-warning/30 bg-status-warning-bg/30 px-3 py-2.5"
          >
            <AlertTriangle size={16} className="text-status-warning flex-shrink-0 mt-0.5" />
            <div className="min-w-0">
              <p className="text-xs font-medium text-content-primary">
                {label} downloads are temporarily unavailable
              </p>
              {message && <p className="text-[11px] text-content-secondary mt-0.5">{message}</p>}
            </div>
          </div>
        );
      })}
    </div>
  );
}
