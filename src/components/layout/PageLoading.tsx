// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file What a page shows for the moment its code is still loading.
 *
 * The Help, Updates and Settings pages are loaded when first opened rather
 * than at start-up (polish pass M12): together with the help text and the
 * Markdown and HTML libraries only Help and Updates use, they were about
 * half of the one script MeedyaDL loaded before showing anything. The
 * first time one of them is opened there is a short wait; this fills it
 * with the page's own header and the app's usual spinner, so the screen
 * does not flash empty and the header does not jump when the page
 * arrives.
 */

import { LoadingSpinner } from '@/components/common/LoadingSpinner';
import { PageHeader } from './PageHeader';

export function PageLoading({ title }: { title: string }) {
  return (
    <div className="flex flex-col h-full">
      <PageHeader title={title} />
      <div className="flex flex-1 items-center justify-center p-6">
        <LoadingSpinner size="md" label="Loading…" />
      </div>
    </div>
  );
}
