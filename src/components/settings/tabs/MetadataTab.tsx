/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file MetadataTab.tsx -- Metadata enrichment settings tab.
 *
 * Renders the "Metadata" tab within the {@link SettingsPage} component.
 * This tab configures post-download metadata enrichment features:
 *
 *   - **Automatic Tags** -- Always-on metadata enrichment (codec tags,
 *     source tags, channel configuration), plus a toggle to control
 *     whether extra API metadata (normalization, spatial properties) is
 *     fetched. Maps to `settings.fetch_extra_tags`.
 *
 *   - **AcoustID Fingerprinting** (opt-in) -- When enabled, generates
 *     Chromaprint audio fingerprints using the embedded fingerprinting engine and looks up AcoustID
 *     identifiers from acoustid.org. Writes `Acoustid Id` and
 *     `Acoustid Fingerprint` freeform atoms. Maps to
 *     `settings.acoustid_enabled`. Release builds ship with a built-in
 *     API key; users can optionally override it with their own.
 *
 *   - **ReplayGain Analysis** (opt-in) -- When enabled, analyses audio
 *     loudness using FFmpeg's EBU R128 filter and writes non-destructive
 *     ReplayGain metadata tags (`replaygain_track_gain`,
 *     `replaygain_track_peak`). Maps to `settings.replaygain_enabled`.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore`.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store
 */

import { useState } from 'react';

// Zustand store for reading/writing metadata enrichment settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Shared form components: Toggle for boolean switches, Input for text fields.
import { Input, Toggle } from '@/components/common';

// Tauri commands for metadata features.
import { auditApiFields } from '@/lib/tauri-commands';

// Types for API audit results.
import type { ApiAuditResult } from '@/types';

/**
 * MetadataTab -- Renders the Metadata settings tab.
 *
 * Contains three sections: an informational block about automatic tags,
 * an AcoustID toggle, and a ReplayGain toggle. The automatic tags
 * section is read-only (no controls) since those tags are always written
 * when applicable.
 */
export function MetadataTab() {
  /** Current settings snapshot */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting metadata setting changes */
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  /** API audit state */
  const [auditUrl, setAuditUrl] = useState('');
  const [auditLoading, setAuditLoading] = useState(false);
  const [auditResult, setAuditResult] = useState<ApiAuditResult | null>(null);
  const [auditError, setAuditError] = useState<string | null>(null);
  const [auditExpanded, setAuditExpanded] = useState(false);

  /** Whether MusicKit credentials are configured (required for audit) */
  const hasMusicKitCredentials =
    !!settings.musickit_team_id?.trim() && !!settings.musickit_key_id?.trim();

  /** Run the API field audit */
  const handleAudit = async () => {
    if (!auditUrl.trim()) return;
    setAuditLoading(true);
    setAuditError(null);
    setAuditResult(null);
    try {
      const result = await auditApiFields(auditUrl.trim());
      setAuditResult(result);
    } catch (err) {
      setAuditError(typeof err === 'string' ? err : String(err));
    } finally {
      setAuditLoading(false);
    }
  };

  return (
    <div className="space-y-6 max-w-xl">
      {/* ================================================================
          Section 1: Automatic Tags
          ================================================================ */}
      <div>
        <h3 className="text-base font-semibold text-content-primary mb-2">Automatic Tags</h3>
        <p className="text-sm text-content-secondary leading-relaxed mb-2">
          MeedyaDL automatically enriches downloaded files with metadata after every download. Codec
          tags (lossless, spatial audio), source tags, and channel configuration are always written.
        </p>
        <p className="text-sm text-content-tertiary leading-relaxed mb-4">
          API-derived tags (ISRC, UPC, genre, advisory ratings, artist IDs) require MusicKit
          credentials. Configure them in Settings &gt; Advanced &gt; API Credentials.
        </p>

        <Toggle
          label="Fetch Extra Tags"
          description="Fetch additional metadata from Apple Music (normalization, spatial/lossless properties, smooth playback info). Adds a small delay per track. Disable if you only want basic codec/source tags."
          checked={settings.fetch_extra_tags}
          onChange={(checked) => updateSettings({ fetch_extra_tags: checked })}
        />

        <Toggle
          label="Content Advisory in Filenames"
          description="Append [Explicit] or [Clean] to album folder names and track filenames based on Apple Music content ratings. Useful for distinguishing explicit and clean versions of the same album."
          checked={settings.content_advisory_in_filenames}
          onChange={(checked) => updateSettings({ content_advisory_in_filenames: checked })}
        />
      </div>

      {/* ================================================================
          Section 2: AcoustID Fingerprinting (opt-in)
          ================================================================ */}
      <div>
        <h3 className="text-base font-semibold text-content-primary mb-4">
          AcoustID Fingerprinting
        </h3>

        <div className="space-y-4">
          <Toggle
            label="Enable AcoustID Fingerprinting"
            description="Generate audio fingerprints and look up AcoustID identifiers for each track. Enables music identification via MusicBrainz. Processes each file individually."
            checked={settings.acoustid_enabled}
            onChange={(checked) => updateSettings({ acoustid_enabled: checked })}
          />

          {settings.acoustid_enabled && (
            <p className="text-xs text-content-tertiary">
              Release builds include a built-in API key. To use your own key, configure it in
              Settings &gt; Advanced &gt; API Credentials.
            </p>
          )}
        </div>
      </div>

      {/* ================================================================
          Section 3: ReplayGain Analysis (opt-in)
          ================================================================ */}
      <div>
        <h3 className="text-base font-semibold text-content-primary mb-4">ReplayGain Analysis</h3>

        <div className="space-y-4">
          <Toggle
            label="Enable ReplayGain Analysis"
            description="Analyse audio loudness and embed non-destructive ReplayGain metadata for volume normalisation. Uses FFmpeg (already installed). Analyses each file individually."
            checked={settings.replaygain_enabled}
            onChange={(checked) => updateSettings({ replaygain_enabled: checked })}
          />
        </div>
      </div>

      {/* ================================================================
          Section 4: API Field Audit (developer diagnostic tool)
          ================================================================ */}
      <div>
        <button
          type="button"
          className="flex items-center gap-2 text-base font-semibold text-content-primary mb-2"
          onClick={() => setAuditExpanded(!auditExpanded)}
        >
          <span className="text-sm text-content-tertiary">{auditExpanded ? '▼' : '▶'}</span>
          API Field Audit
        </button>
        <p className="text-sm text-content-tertiary leading-relaxed mb-4">
          Developer tool: fetch an album from the Apple Music API and compare its fields against the
          known tag definitions in tags.toml. Discovers new or unknown API fields.
        </p>

        {auditExpanded && (
          <div className="space-y-4">
            <div className="flex gap-2">
              <Input
                label=""
                value={auditUrl}
                placeholder="https://music.apple.com/us/album/.../1234567890"
                onChange={(e) => setAuditUrl(e.target.value)}
                className="flex-1"
              />
              <button
                type="button"
                className="px-4 py-2 text-sm font-medium rounded-md bg-accent text-white hover:bg-accent-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed self-end"
                onClick={handleAudit}
                disabled={auditLoading || !auditUrl.trim() || !hasMusicKitCredentials}
              >
                {auditLoading ? 'Auditing...' : 'Audit'}
              </button>
            </div>

            {!hasMusicKitCredentials && (
              <p className="text-sm text-status-warning">
                MusicKit credentials required. Configure Team ID and Key ID in Settings &gt; Cover
                Art.
              </p>
            )}

            {auditError && (
              <p className="text-sm text-status-error">{auditError}</p>
            )}

            {auditResult && (
              <div className="space-y-3 text-sm">
                <div className="flex gap-4 flex-wrap">
                  <span className="text-content-primary font-medium">
                    {auditResult.album_name ?? auditResult.album_id}
                  </span>
                  <span className="text-content-tertiary">
                    {auditResult.track_count} tracks
                  </span>
                </div>

                <div className="flex gap-4 flex-wrap text-sm">
                  <span className="px-2 py-0.5 rounded bg-green-500/20 text-green-400">
                    {auditResult.known_fields.length} known
                  </span>
                  <span className="px-2 py-0.5 rounded bg-amber-500/20 text-amber-400">
                    {auditResult.unknown_fields.length} unknown
                  </span>
                  <span className="px-2 py-0.5 rounded bg-gray-500/20 text-gray-400">
                    {auditResult.missing_fields.length} missing
                  </span>
                </div>

                {auditResult.unknown_fields.length > 0 && (
                  <div>
                    <h4 className="text-sm font-medium text-amber-400 mb-1">
                      Unknown Fields (not in tags.toml)
                    </h4>
                    <div className="max-h-48 overflow-y-auto bg-surface-secondary rounded p-2 space-y-1">
                      {auditResult.unknown_fields.map((field) => (
                        <div
                          key={`${field.scope}-${field.json_path}`}
                          className="text-xs font-mono text-content-secondary"
                        >
                          <span className="text-content-tertiary">[{field.scope}]</span>{' '}
                          {field.json_path}{' '}
                          <span className="text-content-tertiary">({field.value_type})</span>
                          {field.sample_value && (
                            <span className="text-content-tertiary ml-1">
                              = {field.sample_value.length > 60
                                ? `${field.sample_value.slice(0, 60)}...`
                                : field.sample_value}
                            </span>
                          )}
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {auditResult.missing_fields.length > 0 && (
                  <div>
                    <h4 className="text-sm font-medium text-gray-400 mb-1">
                      Missing Fields (in tags.toml but not in API response)
                    </h4>
                    <div className="text-xs font-mono text-content-tertiary">
                      {auditResult.missing_fields.join(', ')}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
