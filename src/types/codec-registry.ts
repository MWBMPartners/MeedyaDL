/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file codec-registry.ts -- TypeScript types for the universal codec registry.
 *
 * Mirrors the Rust `models/codec_registry.rs` structs for use in the React
 * frontend. These types represent the codec definitions loaded from
 * `codecs.toml` and are used for building service-aware UI components
 * (codec selectors, fallback chain editors, quality settings).
 *
 * ## v2 Groundwork
 *
 * These types are foundational for the multi-service architecture (v2+).
 * Currently the app uses `SongCodec` (a simple string union type tied to
 * GAMDL's CLI flags). The registry types provide a richer abstraction with:
 * - Per-service CLI flag mappings
 * - Meta codecs that resolve per-service (e.g., "lossless" → ALAC or FLAC)
 * - MIME types for format identification
 * - Lyrics/subtitle format definitions
 *
 * @see {@link src-tauri/codecs.toml}                   -- Codec definitions (TOML)
 * @see {@link src-tauri/src/models/codec_registry.rs}  -- Rust registry module
 */

// ============================================================
// Audio Codec Types
// ============================================================

/** A concrete audio codec with per-service CLI flag mappings. */
export interface AudioCodecEntry {
  /** Canonical MeedyaDL codec ID (e.g., "eac3-atmos", "alac", "aac-hq"). */
  id: string;
  /** Human-readable display name (e.g., "Dolby Atmos (E-AC-3)"). */
  name: string;
  /** UI grouping category: "spatial", "lossless", or "lossy". */
  category: 'spatial' | 'lossless' | 'lossy';
  /** Whether this codec is lossless (preserves original quality). */
  lossless: boolean;
  /** MIME type (e.g., "audio/alac", "audio/aac"). */
  mimetype: string | null;
  /** Filename suffix for companion download disambiguation (e.g., "[Lossless]").
   *  Empty string means the codec uses clean filenames. Null if not defined. */
  suffix: string | null;
  /** Per-service CLI flag mappings. Key = service ID, value = CLI flag. */
  services: Record<string, string>;
}

// ============================================================
// Meta Codec Types
// ============================================================

/** A meta codec that resolves to a concrete codec per service. */
export interface MetaCodecEntry {
  /** Meta codec ID (e.g., "lossless", "atmos", "best-lossy"). */
  id: string;
  /** Human-readable display name (e.g., "Lossless (Best Available)"). */
  name: string;
  /** Category hint. */
  category: string;
  /** Per-service resolution. Key = service ID, value = concrete audio codec ID. */
  resolves_to: Record<string, string>;
}

// ============================================================
// Video Codec Types
// ============================================================

/** A video codec with per-service CLI flag mappings. */
export interface VideoCodecEntry {
  /** Canonical video codec ID (e.g., "h265", "h264", "vp9"). */
  id: string;
  /** Human-readable display name (e.g., "H.265 (HEVC)"). */
  name: string;
  /** Category: "modern" or "compatible". */
  category: 'modern' | 'compatible';
  /** MIME type (e.g., "video/hevc", "video/avc"). */
  mimetype: string | null;
  /** Per-service CLI flag mappings. */
  services: Record<string, string>;
}

// ============================================================
// Lyrics / Subtitle Format Types
// ============================================================

/** A lyrics/subtitle/caption format entry. */
export interface LyricsFormatEntry {
  /** Format ID (e.g., "ttml", "lrc", "enhanced-lrc", "srt", "vtt"). */
  id: string;
  /** Human-readable display name. */
  name: string;
  /** Category: "xml" or "text". */
  category: 'xml' | 'text';
  /** File extension without dot (e.g., "ttml", "lrc", "srt"). */
  extension: string;
  /** MIME type (e.g., "application/ttml+xml", "text/x-lrc"). */
  mimetype: string;
  /** Whether this format supports word-level timing. */
  word_timing: boolean;
  /** Per-service CLI flag mappings. */
  services: Record<string, string>;
}

// ============================================================
// Full Registry
// ============================================================

/** The complete codec registry loaded from codecs.toml. */
export interface CodecRegistry {
  /** All concrete audio codec entries. */
  audio: AudioCodecEntry[];
  /** All meta (abstract) codec entries. */
  meta: MetaCodecEntry[];
  /** All video codec entries. */
  video: VideoCodecEntry[];
  /** All lyrics/subtitle format entries. */
  lyrics: LyricsFormatEntry[];
}
