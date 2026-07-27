// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file FeatureNoticeBanner — surfaces server-authored notices for
 * features that have been switched off or are running in a degraded
 * state, so a feature never simply vanishes with no explanation.
 *
 * Reads from `useFeatureFlagStore` (`src/stores/featureFlagStore.ts`),
 * which in turn wraps the `FeatureFlagsSnapshot` IPC contract mirrored
 * in `src/types/feature-flags.ts` from
 * `src-tauri/src/models/feature_flags.rs` (commit c4a2185b).
 *
 * ## SECURITY — notice text is untrusted remote data
 *
 * `FlagNotice.message` originates from the feature-flag server. The
 * Rust model's header comment is explicit that the wire schema is
 * deliberately permissive and will keep growing — nothing about the
 * payload, including this string's content, is something this build
 * controls. It is rendered EXCLUSIVELY as a plain React text child
 * (`{notice.message}` below) — never through `dangerouslySetInnerHTML`,
 * never through a markdown renderer, never assigned to `.innerHTML`.
 * React escapes text-node children unconditionally, so a message
 * containing `<script>alert(1)</script>` or markdown syntax renders as
 * inert literal text on screen — it cannot inject an element or execute.
 * Do not "improve" this with rich-text rendering without a dedicated
 * sanitizer AND a security review first; `pr-security.yml` check 4
 * (dangerous frontend sinks) exists specifically to catch this class of
 * regression. Length is also defensively clamped (`clampMessage`) —
 * untrusted data should never be trusted to be a reasonable size either.
 *
 * `notice.url` is deliberately NEVER rendered or opened by this
 * component — see the model's doc comment: "Consumers must validate the
 * scheme before opening it." That validation doesn't exist yet; wiring
 * the link is an explicit follow-up, not an oversight.
 *
 * ## INVARIANT — fetch failures are silent
 *
 * `FeatureFlagsSnapshot.meta` (source / consecutive_failures /
 * last_error) is diagnostics-only and MUST NEVER drive anything
 * user-visible — see the struct-level doc comment on
 * `FeatureFlagsSnapshot` in the Rust model, and the matching comment on
 * `useFeatureFlagStore`. This component derives its entire output from
 * `verdicts` alone, via `selectNoticeEntries` — it never reads `.meta`.
 * A user whose network is down (or who just hit the backend's 1/min
 * rate limit) sees exactly the same banners as before the failed
 * refresh; the backend already logs the failure to the Activity Log.
 * There is no code path in this file that could turn a fetch failure
 * into a banner, because no fetch-failure signal is ever read here.
 */

import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertCircle, AlertTriangle, Info } from 'lucide-react';

import { useFeatureFlagStore, selectNoticeEntries } from '@/stores/featureFlagStore';
import type { FlagVerdict, NoticeSeverity } from '@/types/feature-flags';

/**
 * Defensive clamp on server-authored notice text (hard rule from the
 * feature spec). Untrusted remote data should never be trusted to be a
 * sane length either, independent of the injection concerns above.
 */
const MAX_NOTICE_LENGTH = 500;

/** The app's closed set of status design tokens — see base.css `--status-*`. */
type StatusToken = 'info' | 'warning' | 'error';

/**
 * Maps a wire severity onto {@link StatusToken}. `unknown` (a severity
 * string this build doesn't recognise) degrades to `info`, matching
 * `NoticeSeverity::render_as` on the Rust side. Never invent additional
 * colours here — the a11y colour-blind themes key off exactly these
 * three tokens.
 */
function severityToToken(severity: NoticeSeverity): StatusToken {
  switch (severity) {
    case 'critical':
      return 'error';
    case 'maintenance':
    case 'deprecated':
      return 'warning';
    case 'info':
    case 'unknown':
    default:
      return 'info';
  }
}

/** Tailwind classes for each status token, matching ToastContainer's convention. */
const TOKEN_CLASSES: Record<StatusToken, { bg: string; border: string; text: string }> = {
  info: { bg: 'bg-status-info-bg', border: 'border-status-info', text: 'text-status-info' },
  warning: {
    bg: 'bg-status-warning-bg',
    border: 'border-status-warning',
    text: 'text-status-warning',
  },
  error: { bg: 'bg-status-error-bg', border: 'border-status-error', text: 'text-status-error' },
};

/** One icon per status token, matching ToastContainer's icon choices. */
const TOKEN_ICONS: Record<StatusToken, typeof Info> = {
  info: Info,
  warning: AlertTriangle,
  error: AlertCircle,
};

/** Clamp displayed length. Adds an ellipsis when truncated. */
function clampMessage(message: string): string {
  if (message.length <= MAX_NOTICE_LENGTH) return message;
  return `${message.slice(0, MAX_NOTICE_LENGTH)}…`;
}

/** A single resolved, display-ready notice — one per banner row. */
interface ResolvedNotice {
  key: string;
  token: StatusToken;
  message: string;
}

/**
 * Resolves the display copy + status token for one flag's verdict.
 *
 * - A non-empty server message always wins, rendered verbatim (clamped),
 *   styled by its own `severity` — this covers both "disabled with a
 *   real explanation" and "still enabled but the server wants to say
 *   something about it" (e.g. a maintenance heads-up).
 * - A disabled feature with no usable message (notice is null, or its
 *   message is blank/whitespace-only) gets the i18n fallback copy — the
 *   case that stops "the feature just vanished" from reading as
 *   breakage. Uses the flag's `label` when present, else the raw key.
 * - An enabled feature whose notice has a blank message has nothing
 *   worth showing and is dropped (returns `null`) — it only reached this
 *   function because `selectNoticeEntries` can't see the message is
 *   empty without unpacking `notice` itself.
 */
function resolveNotice(
  key: string,
  verdict: FlagVerdict,
  fallbackText: (feature: string) => string
): ResolvedNotice | null {
  const rawMessage = verdict.notice?.message.trim();

  if (rawMessage) {
    return {
      key,
      token: severityToToken(verdict.notice!.severity),
      message: clampMessage(rawMessage),
    };
  }

  if (!verdict.enabled) {
    const featureLabel = verdict.label ?? key;
    return {
      key,
      // No server severity to key off when there's no usable notice —
      // 'unknown' renders as the mildest (info) token, matching the
      // deliberately calm tone of the fallback copy itself.
      token: severityToToken('unknown'),
      message: fallbackText(featureLabel),
    };
  }

  return null;
}

/**
 * Renders one banner per feature that is disabled or carries a
 * server-authored notice. Renders nothing (`null`) when there is
 * nothing to show for the current snapshot — including whenever the
 * only thing that changed since the last render was a failed background
 * refresh (see the fetch-failure invariant in this file's header).
 */
export function FeatureNoticeBanner() {
  const { t } = useTranslation();

  // Subscribe to the whole snapshot's `data` — see the invariant above:
  // this is the ONLY field of the store this component ever reads.
  // `.meta` is never touched anywhere in this file.
  const snapshotData = useFeatureFlagStore((s) => s.data);

  const entries = useMemo(() => selectNoticeEntries(snapshotData), [snapshotData]);

  const notices = useMemo(() => {
    const fallbackText = (feature: string) => t('featureFlags.fallbackMessage', { feature });
    return entries
      .map((entry) => resolveNotice(entry.key, entry.verdict, fallbackText))
      .filter((n): n is ResolvedNotice => n !== null);
  }, [entries, t]);

  if (notices.length === 0) return null;

  return (
    <div className="flex flex-col gap-2 mx-4 mt-3 mb-1" data-testid="feature-notice-banner-list">
      {notices.map((notice) => {
        const classes = TOKEN_CLASSES[notice.token];
        const Icon = TOKEN_ICONS[notice.token];
        return (
          <div
            key={notice.key}
            role="status"
            className={`flex items-start gap-3 p-3 rounded-platform border ${classes.bg} ${classes.border}`}
            data-testid="feature-notice-banner"
          >
            <Icon size={18} className={`flex-shrink-0 mt-0.5 ${classes.text}`} aria-hidden="true" />
            {/*
             * SECURITY: `notice.message` may be untrusted remote text (or
             * the local i18n fallback). It is rendered ONLY as a plain
             * text child here — see the file-level doc comment. Do not
             * change this to any HTML-interpreting render path.
             */}
            <p className={`text-sm ${classes.text} break-words`}>{notice.message}</p>
          </div>
        );
      })}
    </div>
  );
}

export default FeatureNoticeBanner;
