// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Service-agnostic metadata provider trait (#351).
// =================================================
//
// Defines a common interface for fetching album/track metadata from
// any media service (Apple Music, iTunes, Spotify, YouTube, etc.).
// The enrichment pipeline calls providers through this trait, enabling
// multi-service metadata fetching without coupling to specific APIs.
//
// ## Architecture
//
// ```
// Enrichment Pipeline
//   |
//   +-- MetadataProvider::fetch_album(album_id) -> AlbumMetadata
//   +-- MetadataProvider::fetch_tracks(album_id) -> Vec<TrackMetadata>
//   +-- MetadataProvider::provider_name() -> "apple-music" | "itunes" | ...
//   +-- MetadataProvider::requires_auth() -> bool
// ```
//
// ## Implementations
//
// - AppleMusicProvider: MusicKit JWT auth, rich metadata (editorialNotes,
//   audioTraits, animated artwork). See apple_music_api.rs.
// - ItunesProvider: No auth, supplementary fields (price, country, discCount).
//   See apple_music_api.rs fetch_itunes_lookup().
// - SpotifyProvider: (future) OAuth2 auth via votify
// - YouTubeProvider: (future) Public API + yt-dlp metadata
//
// @see models/tag_registry.rs — Tag definitions driven by meedya-core
// @see services/metadata_tag_service.rs — Consumes provider output

use super::apple_music_api::AlbumMetadata;

/// Provider priority for metadata field merging.
/// Higher priority providers overwrite lower priority fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProviderPriority {
    /// Lowest priority — supplementary data only (iTunes Lookup)
    Supplementary = 0,
    /// Standard priority — primary metadata source (Apple Music)
    Primary = 1,
    /// Highest priority — user-provided overrides
    UserOverride = 2,
}

/// Service-agnostic metadata provider trait.
///
/// Each media service implements this trait to provide album and track
/// metadata for the enrichment pipeline. Providers are called in
/// priority order (lowest first), with higher-priority providers
/// overwriting fields from lower-priority ones.
///
/// ## Dyn compatibility
///
/// `async fn` in traits is not dyn-compatible, so `fetch_album_metadata`
/// returns a pinned boxed future instead. This allows storing providers
/// as `Box<dyn MetadataProvider>` in the registry.
pub trait MetadataProvider: Send + Sync {
    /// Human-readable name of this provider (e.g., "Apple Music", "iTunes")
    fn provider_name(&self) -> &str;

    /// The MediaServiceId this provider serves
    fn service_id(&self) -> &str;

    /// Provider priority for field merging
    fn priority(&self) -> ProviderPriority;

    /// Whether this provider requires authentication credentials
    fn requires_auth(&self) -> bool;

    /// Fetch album metadata by album ID.
    ///
    /// Returns `Ok(Some(metadata))` on success, `Ok(None)` if not found,
    /// `Err(message)` on network/auth errors.
    fn fetch_album_metadata(
        &self,
        album_id: &str,
        storefront: &str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<AlbumMetadata>, String>> + Send + '_>,
    >;
}

/// Registry of available metadata providers.
///
/// Providers are stored in priority order (lowest first). The enrichment
/// pipeline iterates providers and merges results, with higher-priority
/// providers overwriting lower-priority fields.
pub struct MetadataProviderRegistry {
    providers: Vec<Box<dyn MetadataProvider>>,
}

impl MetadataProviderRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a provider. Providers are sorted by priority after insertion.
    pub fn register(&mut self, provider: Box<dyn MetadataProvider>) {
        self.providers.push(provider);
        self.providers.sort_by_key(|p| p.priority());
    }

    /// Get all registered providers in priority order.
    pub fn providers(&self) -> &[Box<dyn MetadataProvider>] {
        &self.providers
    }

    /// Get the number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl Default for MetadataProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
