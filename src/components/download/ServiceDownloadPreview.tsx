// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file ServiceDownloadPreview (#911-9) — per-service "what gets
 *       downloaded" card surfaced under the Download page URL textarea
 *       once `detectService(url)` resolves.
 *
 * Different services produce wildly different artifacts:
 *   - Apple Music: audio + lyrics (multiple sidecar formats) +
 *     animated artwork + companion codecs + music video relations
 *   - BBC iPlayer (M8, planned): video + subtitles
 *   - Spotify (M9, planned): Ogg audio + lyrics
 *   - YouTube (M10, planned): video / audio depending on URL
 *
 * The card lists exactly what *this* user's current settings will produce
 * for the pasted URL's service — so the user can see ahead of time
 * "you're about to download lossless audio, Enhanced LRC lyrics, animated
 * artwork, and 1 companion codec" before hitting "Add to Queue."
 *
 * The list is rendered as a compact bullet-grid with icons. The card
 * uses `bg-accent/5 border-accent/30` to match the visual weight of
 * the existing cookie-warning + multi-URL chip patterns in
 * `DownloadForm.tsx`. Hidden when no URL is present, when the URL
 * isn't parseable, or when the resolved service has no consumer here
 * (the M8-M10 stub services show a "coming soon" line).
 *
 * **Anti-pattern 3 reminder** (from
 * `.claude/memory/project_multi_service_ui_direction.md`): pasting a
 * service URL must NOT auto-open the service's Settings tab. This
 * card is the explicitly-approved alternative — it shows what the
 * service will do given the current settings, without hijacking the
 * user's flow mid-paste. Future enhancement could turn each artifact
 * line into a settings deep-link, but the *default* path stays
 * non-disruptive.
 */

import { useMemo } from 'react';
import {
  Disc3,
  Film,
  Image as ImageIcon,
  Languages,
  Music,
  Subtitles,
  Video,
} from 'lucide-react';

import { detectPlatform } from '@/lib/platform-config';
import { PlatformIcon } from '@/lib/PlatformIcon';
import { useSettingsStore } from '@/stores/settingsStore';
import { MEDIA_SERVICE_LABELS, SONG_CODEC_LABELS, type MediaServiceId, type SongCodec } from '@/types';

interface PreviewArtifact {
  icon: typeof Music;
  label: string;
}

/**
 * Compute the artifact bullet-list for an Apple Music URL given the
 * current settings store. Each `PreviewArtifact` is one line of the
 * preview card. Order: audio first, then per-track adjuncts (lyrics,
 * subtitles), then per-album adjuncts (artwork), then companions.
 *
 * Exposed as a top-level helper so future BBC iPlayer / Spotify /
 * YouTube cards can be added without coupling to this one.
 */
function buildAppleMusicArtifacts(
  settings: ReturnType<typeof useSettingsStore.getState>['settings']
): PreviewArtifact[] {
  const items: PreviewArtifact[] = [];

  // Audio with codec label
  const codec = settings.default_song_codec as SongCodec | undefined;
  const codecLabel = codec ? (SONG_CODEC_LABELS[codec] ?? codec) : 'Audio';
  items.push({ icon: Music, label: codecLabel });

  // Lyrics — embedded into the audio file when GAMDL writes the
  // `\xa9lyr` atom; the sidecar / synced variants follow below.
  // (Apple Music API supplies synced TTML on every track that has
  // lyrics; for non-lyrics tracks all of these silently skip.)
  const wantsSyncedLyrics = settings.enhanced_lrc || settings.generate_rich_srt;
  const wantsAnyLyrics =
    wantsSyncedLyrics ||
    settings.generate_webvtt ||
    settings.generate_ass ||
    settings.generate_lyricsfile ||
    settings.keep_lyrics_sidecar;
  if (wantsAnyLyrics) {
    const formats: string[] = [];
    if (settings.enhanced_lrc) formats.push('Enhanced LRC');
    if (settings.generate_rich_srt) formats.push('Rich SRT');
    if (settings.generate_webvtt) formats.push('WebVTT');
    if (settings.generate_ass) formats.push('ASS');
    if (settings.generate_lyricsfile) formats.push('Lyricsfile (.lyrics)');
    if (formats.length === 0 && settings.keep_lyrics_sidecar) {
      formats.push('LRC');
    }
    items.push({
      icon: settings.enhanced_lrc ? Languages : Subtitles,
      label: `Lyrics — ${formats.join(' + ')}`,
    });
  }

  // Static cover art is always written by GAMDL itself; no toggle.
  items.push({ icon: ImageIcon, label: 'Cover art' });

  // Animated artwork (optional)
  if (settings.animated_artwork_enabled) {
    items.push({ icon: Film, label: 'Animated artwork' });
  }

  // Companion codecs — only when `companion_mode` ≠ 'disabled'.
  // The exact codec list depends on the mode (atmos_to_lossless ships
  // a Lossless companion alongside the primary Atmos track; etc.); we
  // surface the mode name so a user with a specific preference knows
  // which one is wired without having to read the docs.
  const companionMode = settings.companion_mode;
  if (companionMode && companionMode !== 'disabled') {
    const friendly: Record<string, string> = {
      atmos_to_lossless: 'Companion: Lossless (when Atmos)',
      atmos_to_lossless_and_lossy: 'Companion: Lossless + Lossy (when Atmos)',
      specialist_to_lossy: 'Companion: Lossy (when specialist codec)',
      atmos_to_all_formats: 'Companion: every format (when Atmos)',
      custom: 'Companion: custom codec list',
    };
    const label = friendly[companionMode] ?? `Companion: ${companionMode}`;
    items.push({ icon: Disc3, label });
  }

  // Music video relations (optional, requires MusicKit credentials)
  if (settings.music_video_companion) {
    items.push({ icon: Video, label: 'Music video relations (when present)' });
  }

  // .meedyadl manifest is always written by the enrichment task —
  // intentional ephemeral; not surfaced here because users don't
  // think about it as a "thing they're downloading."

  return items;
}

/**
 * Compute the preview artifacts for the resolved service. Falls back
 * to a "coming soon" sentinel for services that aren't yet wired
 * end-to-end (BBC iPlayer, Spotify, YouTube — covered by the
 * stub-service modules from `prep/expanded-services-groundwork`).
 */
function buildArtifacts(
  service: MediaServiceId,
  settings: ReturnType<typeof useSettingsStore.getState>['settings']
): PreviewArtifact[] | 'coming-soon' {
  if (service === 'apple-music') {
    return buildAppleMusicArtifacts(settings);
  }
  return 'coming-soon';
}

export interface ServiceDownloadPreviewProps {
  /**
   * The resolved service ID from `detectService(url)`. When `null`
   * (no URL pasted yet, or unrecognised host), the card renders
   * nothing — the existing helper text under the textarea handles
   * the empty state.
   */
  service: MediaServiceId | null;
  /**
   * The current URL list — used purely to resolve the platform icon
   * via the existing `detectPlatform()` helper. Pass `urls?.slice(0, 1)`
   * if the textarea contains multi-URL input; single representative
   * URL is enough for the preview.
   */
  urls: string[];
}

/**
 * Render the per-service preview card. Returns `null` when there's
 * nothing to show (no service, or service has no consumer here yet
 * AND the user is on a stable channel — the "coming soon" line is
 * gated behind dev access).
 */
export function ServiceDownloadPreview({
  service,
  urls,
}: ServiceDownloadPreviewProps) {
  const settings = useSettingsStore((s) => s.settings);
  const artifacts = useMemo(
    () => (service ? buildArtifacts(service, settings) : null),
    [service, settings]
  );
  const platform = detectPlatform(urls);

  if (!service || !artifacts) return null;

  const serviceLabel = MEDIA_SERVICE_LABELS[service];

  if (artifacts === 'coming-soon') {
    return (
      <div
        className="flex items-start gap-2 p-3 rounded-platform bg-accent/5 border border-accent/30 text-xs text-content-secondary"
        data-testid="service-download-preview-coming-soon"
      >
        <PlatformIcon platform={platform} size={16} />
        <div>
          <p className="text-content-primary font-medium mb-0.5">
            {serviceLabel} support is coming
          </p>
          <p>
            URL parsing recognises this service, but the download engine
            isn&rsquo;t wired yet. Track progress in the multi-service EPIC.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div
      className="rounded-platform border border-accent/30 bg-accent/5 p-3"
      data-testid="service-download-preview"
      aria-label={`${serviceLabel} download preview`}
    >
      <div className="flex items-center gap-2 mb-2">
        <PlatformIcon platform={platform} size={16} />
        <p className="text-xs font-medium text-content-primary">
          {serviceLabel} will download:
        </p>
      </div>
      <ul className="grid grid-cols-1 md:grid-cols-2 gap-x-4 gap-y-1.5 text-xs text-content-secondary">
        {artifacts.map((artifact, i) => {
          const Icon = artifact.icon;
          return (
            <li key={i} className="flex items-center gap-1.5">
              <Icon size={12} className="flex-shrink-0 text-content-tertiary" />
              <span className="truncate">{artifact.label}</span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
