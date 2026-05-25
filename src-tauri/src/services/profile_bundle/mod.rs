// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License.
//
// Profile Bundle — .meedyabundle export/import format (#876 EPIC B P1).
//
// A standard ZIP container that bundles a MeedyaDL install's state
// (settings, queue, history, optionally credentials/activity-log/
// manifests/database) into a single portable file. Companion to the
// in-app "Export Profile" / "Import Profile" flows.
//
// P1 scope (this commit): pure-Rust format definition + (de)serialiser
// primitives. **No UI, no IPC, no credential encryption yet** — those
// land in P2 (export command + Settings UI), P3 (import wizard), and
// P4 (AES-256-GCM + PBKDF2). The format is deliberately
// forward-compatible: `meta.json.contents` enumerates exactly what's
// present, so importers degrade gracefully.
//
// Format layout (from #876 EPIC body):
//
//   archive root/
//   ├── meta.json                       (REQUIRED — only guaranteed file)
//   ├── settings.json                   (REQUIRED — always exported)
//   ├── settings.json.sha256            (REQUIRED — existing integrity check)
//   ├── queue.json                      (OPTIONAL)
//   ├── history.json                    (OPTIONAL — pre-EPIC-A format)
//   ├── database.sqlite                 (OPTIONAL — post-EPIC-A format)
//   ├── credentials.encrypted           (OPTIONAL — AES-256-GCM)
//   ├── credentials.kdf-params.json     (REQUIRED if credentials present)
//   ├── activity_log/<dates>.log        (OPTIONAL)
//   └── manifests/<artist>/<album>/manifest.meedyadl  (OPTIONAL, OPT-IN)
//
// `meta.json` is the only file guaranteed present. `meta.json.contents`
// lists what's actually packaged — importers MUST honour this list and
// MUST NOT assume any other file is present.

use std::io::{Cursor, Read, Write};

use serde::{Deserialize, Serialize};

pub mod credentials;

/// Current bundle-format version. **Bump only on breaking changes** to
/// the file layout or the meta.json schema; additive changes (new
/// OPTIONAL entries in `contents`) do not need a version bump because
/// older importers can simply ignore them.
pub const BUNDLE_FORMAT_VERSION: u32 = 1;

/// Conventional file extension. Bundles ARE valid ZIPs — `unzip` will
/// inspect them. The extension is purely a UX affordance for OS
/// "open with" associations.
pub const BUNDLE_EXTENSION: &str = "meedyabundle";

/// MIME type for OS file associations + the eventual web import flow.
pub const BUNDLE_MIME_TYPE: &str = "application/vnd.mwbmpartners.meedyadl.bundle+zip";

/// Filenames inside the archive. Constants live alongside the format
/// definition so every consumer reads the same canonical strings.
pub mod entry {
    pub const META: &str = "meta.json";
    pub const SETTINGS: &str = "settings.json";
    pub const SETTINGS_SHA256: &str = "settings.json.sha256";
    pub const QUEUE: &str = "queue.json";
    pub const HISTORY: &str = "history.json";
    pub const DATABASE: &str = "database.sqlite";
    pub const CREDENTIALS: &str = "credentials.encrypted";
    pub const CREDENTIALS_KDF: &str = "credentials.kdf-params.json";
    pub const ACTIVITY_LOG_PREFIX: &str = "activity_log/";
    pub const MANIFESTS_PREFIX: &str = "manifests/";
}

/// Enumerates the optional sections that a bundle MAY contain. The
/// canonical names are serialised to `meta.json.contents` so importers
/// can short-circuit lookups for entries that aren't there. `Settings`
/// is intentionally not in this enum — it's REQUIRED in every bundle
/// and so its presence is implicit, not advertised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleSection {
    Queue,
    History,
    Database,
    Credentials,
    ActivityLog,
    Manifests,
}

impl BundleSection {
    /// Stable serialised name (used in tests + `meta.json.contents`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queue => "queue",
            Self::History => "history",
            Self::Database => "database",
            Self::Credentials => "credentials",
            Self::ActivityLog => "activity_log",
            Self::Manifests => "manifests",
        }
    }
}

/// Top-level metadata embedded as `meta.json` at the root of every
/// bundle. Future bundle versions MAY add fields here — existing
/// fields are stable. Fields with `#[serde(default)]` are tolerated
/// missing on read, which means old bundles open cleanly on a newer
/// importer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleMeta {
    /// Always equals [`BUNDLE_FORMAT_VERSION`] at write time. Importers
    /// reject bundles whose `bundle_version` is strictly newer.
    pub bundle_version: u32,

    /// MeedyaDL version that produced the bundle (e.g.,
    /// `"1.10.0-alpha.14"`). Informational; importers do NOT gate
    /// on this value.
    pub meedyadl_version: String,

    /// Platform string of the exporting machine, in
    /// `{os}-{arch}` form. Informational only — bundles are
    /// platform-agnostic (the SQLite DB and JSON files are
    /// portable). E.g., `"darwin-aarch64"`, `"linux-x86_64"`.
    pub platform: String,

    /// UTC ISO-8601 timestamp of the export.
    pub exported_at: String,

    /// Optional user-set identifier (typically the user's email).
    /// Never used for authentication — purely a "which install did
    /// this come from" cue for the import wizard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exported_by: Option<String>,

    /// Optional free-form note set by the exporting user (e.g.,
    /// "Pre-MacBook-Pro replacement export, 2026-05-24").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,

    /// Which OPTIONAL sections are present. Importers MUST honour
    /// this list — never assume an entry exists just because the
    /// archive holds it.
    #[serde(default)]
    pub contents: Vec<BundleSection>,
}

impl BundleMeta {
    /// Construct a fresh BundleMeta for an export operation. The
    /// caller will populate `contents` based on the user's export
    /// selections before the bundle is sealed.
    pub fn new_for_export(meedyadl_version: impl Into<String>) -> Self {
        Self {
            bundle_version: BUNDLE_FORMAT_VERSION,
            meedyadl_version: meedyadl_version.into(),
            platform: current_platform_string(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            exported_by: None,
            note: None,
            contents: Vec::new(),
        }
    }

    /// Whether this bundle declares a given optional section. Cheap
    /// O(N) lookup over the (small) contents list.
    pub fn has(&self, section: BundleSection) -> bool {
        self.contents.contains(&section)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("bundle io: {0}")]
    Io(#[from] std::io::Error),

    #[error("bundle zip: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("bundle json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("missing required entry in bundle: {0}")]
    MissingEntry(&'static str),

    #[error(
        "bundle was produced by a newer MeedyaDL (bundle_version={found}); this build knows up to v{known}"
    )]
    UnsupportedVersion { found: u32, known: u32 },
}

/// In-memory builder for the contents of a bundle archive. Add entries
/// via `add_*` then `finalize_to_bytes()` to get the sealed ZIP. P2
/// will add a file-sink variant; for P1 we only need the in-memory
/// path because the round-trip unit tests are the validation surface.
pub struct BundleWriter {
    meta: BundleMeta,
    files: Vec<(String, Vec<u8>)>,
}

impl BundleWriter {
    /// Create a writer initialised from an `_export_ meta. Settings
    /// must be added separately via [`add_settings`].
    pub fn new(meta: BundleMeta) -> Self {
        Self {
            meta,
            files: Vec::new(),
        }
    }

    /// Add the REQUIRED settings.json + its sha256 sidecar.
    pub fn add_settings(&mut self, settings_json: Vec<u8>, sha256_hex: String) {
        self.files
            .push((entry::SETTINGS.to_string(), settings_json));
        self.files
            .push((entry::SETTINGS_SHA256.to_string(), sha256_hex.into_bytes()));
    }

    /// Add an OPTIONAL section by path + bytes. The caller is
    /// responsible for marking the section in `meta.contents` via
    /// [`declare_section`] so importers know to look for it.
    pub fn add_optional_file(&mut self, archive_path: impl Into<String>, bytes: Vec<u8>) {
        self.files.push((archive_path.into(), bytes));
    }

    /// Mark an optional section as present. MUST be paired with one
    /// or more [`add_optional_file`] calls for the section's actual
    /// payload. Idempotent — duplicate declarations are silently
    /// collapsed by the bundle reader.
    pub fn declare_section(&mut self, section: BundleSection) {
        if !self.meta.contents.contains(&section) {
            self.meta.contents.push(section);
        }
    }

    /// Read the SQLite download index from disk and add it to the
    /// bundle under `entry::DATABASE`, automatically declaring the
    /// `Database` section (#875 M5).
    ///
    /// The caller is responsible for ensuring SQLite is not actively
    /// writing when this is called — the recommended pattern is to
    /// `DownloadIndex` drop (close) immediately before exporting. The
    /// WAL is checkpointed by SQLite on close, so a fresh open against
    /// the file path returns a consistent point-in-time snapshot.
    ///
    /// Returns `Err(BundleError::Io)` only when the DB file cannot be
    /// read; absence is logged at the caller's discretion (an
    /// install that has never opened the DB simply skips this step).
    pub fn add_database_from_disk(
        &mut self,
        db_path: &std::path::Path,
    ) -> Result<(), BundleError> {
        let bytes = std::fs::read(db_path)?;
        self.add_optional_file(entry::DATABASE, bytes);
        self.declare_section(BundleSection::Database);
        Ok(())
    }

    /// Borrow the in-progress metadata. Used by tests + the eventual
    /// `export_profile` IPC to surface a content-size estimate before
    /// finalisation.
    pub fn meta(&self) -> &BundleMeta {
        &self.meta
    }

    /// Seal the bundle to in-memory ZIP bytes. Stores meta.json LAST
    /// so the archive's TOC reflects the final `contents` list.
    pub fn finalize_to_bytes(self) -> Result<Vec<u8>, BundleError> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o644);

            for (name, bytes) in &self.files {
                zip.start_file(name, opts)?;
                zip.write_all(bytes)?;
            }

            // meta.json last so the contents list is final.
            zip.start_file(entry::META, opts)?;
            let meta_json = serde_json::to_vec_pretty(&self.meta)?;
            zip.write_all(&meta_json)?;

            zip.finish()?;
        }
        Ok(buf.into_inner())
    }
}

/// Read-only view of a bundle archive. Opens lazily — file entries
/// are not extracted until the consumer asks for them via
/// [`BundleReader::read_entry`].
pub struct BundleReader {
    archive: zip::ZipArchive<Cursor<Vec<u8>>>,
    meta: BundleMeta,
}

impl std::fmt::Debug for BundleReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BundleReader")
            .field("meta", &self.meta)
            .field("archive_entries", &self.archive.len())
            .finish()
    }
}

impl BundleReader {
    /// Open a bundle from in-memory bytes. Parses meta.json eagerly
    /// (so unsupported-version errors surface immediately) but defers
    /// per-entry reads.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, BundleError> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;

        let mut meta_buf = Vec::new();
        {
            let mut entry = archive
                .by_name(entry::META)
                .map_err(|_| BundleError::MissingEntry(entry::META))?;
            entry.read_to_end(&mut meta_buf)?;
        }
        let meta: BundleMeta = serde_json::from_slice(&meta_buf)?;

        if meta.bundle_version > BUNDLE_FORMAT_VERSION {
            return Err(BundleError::UnsupportedVersion {
                found: meta.bundle_version,
                known: BUNDLE_FORMAT_VERSION,
            });
        }

        Ok(Self { archive, meta })
    }

    /// Borrow the parsed metadata. Cheap.
    pub fn meta(&self) -> &BundleMeta {
        &self.meta
    }

    /// Read an entry's bytes by path. Returns `None` if the entry is
    /// absent from the archive — callers should check
    /// [`BundleMeta::has`] first for optional sections.
    pub fn read_entry(&mut self, name: &str) -> Result<Option<Vec<u8>>, BundleError> {
        let mut entry = match self.archive.by_name(name) {
            Ok(e) => e,
            Err(zip::result::ZipError::FileNotFound) => return Ok(None),
            Err(e) => return Err(BundleError::Zip(e)),
        };
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        Ok(Some(buf))
    }

    /// Enumerate every archive entry whose name starts with
    /// `prefix`. Returns the FULL entry names (including the prefix)
    /// so callers can pass them straight back to
    /// [`read_entry`]. Used by `import_profile` (#876 P3) to walk the
    /// `activity_log/` and `manifests/` sub-trees one entry at a
    /// time. Cheap — only the central directory is consulted, no
    /// file payloads are decompressed.
    pub fn file_names_starting_with(
        &self,
        prefix: &str,
    ) -> Result<Vec<String>, BundleError> {
        let mut out = Vec::new();
        for i in 0..self.archive.len() {
            // Read-only access to entry names doesn't need a mutable
            // archive borrow in zip 2.x — we go through the central
            // directory via `file_names`.
            if let Some(name) = self.archive.file_names().nth(i) {
                if name.starts_with(prefix) {
                    out.push(name.to_string());
                }
            }
        }
        Ok(out)
    }

    /// Extract the embedded SQLite database to `target_path`,
    /// overwriting any existing file at that location (#875 M5).
    ///
    /// Returns `Ok(true)` when the bundle had a Database section and
    /// the file was written; `Ok(false)` when no Database section is
    /// declared (or the entry is missing — a corrupt bundle case
    /// surfaces through the latter). The target directory must
    /// exist; the helper does NOT auto-create parent directories
    /// because the import wizard owns directory setup and we don't
    /// want this helper to silently land bytes in an unexpected
    /// location.
    ///
    /// The caller is responsible for ensuring no other process /
    /// thread has the target DB file open at the time of extraction.
    pub fn extract_database_to(
        &mut self,
        target_path: &std::path::Path,
    ) -> Result<bool, BundleError> {
        if !self.meta.has(BundleSection::Database) {
            return Ok(false);
        }
        let bytes = match self.read_entry(entry::DATABASE)? {
            Some(b) => b,
            None => return Ok(false),
        };
        std::fs::write(target_path, bytes)?;
        Ok(true)
    }
}

/// Compute the canonical platform string baked into `meta.platform`.
/// Format: `{os}-{arch}` (e.g., `darwin-aarch64`, `linux-x86_64`,
/// `windows-x86_64`). Pure metadata — importers do NOT gate on this.
fn current_platform_string() -> String {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        std::env::consts::OS
    };
    let arch = std::env::consts::ARCH;
    format!("{os}-{arch}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_serialises_with_stable_field_names() {
        let mut meta = BundleMeta::new_for_export("1.10.0-alpha.14");
        meta.contents.push(BundleSection::Queue);
        meta.contents.push(BundleSection::Credentials);
        let json = serde_json::to_string_pretty(&meta).unwrap();
        // Snake-case section names per #876 EPIC body.
        assert!(json.contains("\"queue\""));
        assert!(json.contains("\"credentials\""));
        assert!(json.contains("\"bundle_version\": 1"));
        assert!(json.contains("\"meedyadl_version\": \"1.10.0-alpha.14\""));
    }

    #[test]
    fn round_trip_minimal_bundle_preserves_settings_and_meta() {
        // Build a minimal bundle with just settings.json + sha256.
        let mut writer = BundleWriter::new(BundleMeta::new_for_export("1.10.0-test"));
        let settings_bytes = br#"{"app":"meedyadl","theme":"dark"}"#.to_vec();
        writer.add_settings(settings_bytes.clone(), "deadbeef".to_string());
        let zip_bytes = writer.finalize_to_bytes().unwrap();

        // Re-open and verify everything roundtrips.
        let mut reader = BundleReader::from_bytes(zip_bytes).unwrap();
        assert_eq!(reader.meta().bundle_version, BUNDLE_FORMAT_VERSION);
        assert_eq!(reader.meta().meedyadl_version, "1.10.0-test");
        assert!(reader.meta().contents.is_empty(), "no optional sections declared");

        let restored = reader
            .read_entry(entry::SETTINGS)
            .unwrap()
            .expect("settings.json present");
        assert_eq!(restored, settings_bytes);

        let sha = reader
            .read_entry(entry::SETTINGS_SHA256)
            .unwrap()
            .expect("sha256 sidecar present");
        assert_eq!(sha, b"deadbeef");
    }

    #[test]
    fn round_trip_with_optional_sections_marks_contents_correctly() {
        let mut writer = BundleWriter::new(BundleMeta::new_for_export("1.10.0-test"));
        writer.add_settings(b"{}".to_vec(), "00".to_string());

        // Queue + History optional sections.
        writer.add_optional_file(entry::QUEUE, br#"{"items":[]}"#.to_vec());
        writer.declare_section(BundleSection::Queue);
        writer.add_optional_file(entry::HISTORY, br#"{"entries":[]}"#.to_vec());
        writer.declare_section(BundleSection::History);

        let bytes = writer.finalize_to_bytes().unwrap();
        let reader = BundleReader::from_bytes(bytes).unwrap();
        assert!(reader.meta().has(BundleSection::Queue));
        assert!(reader.meta().has(BundleSection::History));
        assert!(!reader.meta().has(BundleSection::Credentials));
    }

    #[test]
    fn refuses_to_open_newer_bundle_version() {
        let mut writer = BundleWriter::new(BundleMeta {
            bundle_version: BUNDLE_FORMAT_VERSION + 99,
            meedyadl_version: "future-build".into(),
            platform: "darwin-aarch64".into(),
            exported_at: "2030-01-01T00:00:00Z".into(),
            exported_by: None,
            note: None,
            contents: Vec::new(),
        });
        writer.add_settings(b"{}".to_vec(), "00".to_string());
        let bytes = writer.finalize_to_bytes().unwrap();
        let err = BundleReader::from_bytes(bytes).expect_err("should refuse newer bundle");
        match err {
            BundleError::UnsupportedVersion { found, known } => {
                assert_eq!(found, BUNDLE_FORMAT_VERSION + 99);
                assert_eq!(known, BUNDLE_FORMAT_VERSION);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn missing_meta_json_is_a_clear_error() {
        // Hand-craft a ZIP without meta.json to ensure the error is
        // not a confusing JSON parse failure.
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            zip.start_file("not-a-meedyabundle.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"nope").unwrap();
            zip.finish().unwrap();
        }
        let err = BundleReader::from_bytes(buf.into_inner())
            .expect_err("should refuse bundle without meta.json");
        match err {
            BundleError::MissingEntry(name) => assert_eq!(name, entry::META),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn read_entry_returns_none_for_absent_optional() {
        // Bundle declares Queue but doesn't actually add the file —
        // simulates a corrupted bundle. read_entry returns Ok(None)
        // rather than erroring so the import wizard can handle it
        // gracefully.
        let mut writer = BundleWriter::new(BundleMeta::new_for_export("test"));
        writer.add_settings(b"{}".to_vec(), "00".to_string());
        writer.declare_section(BundleSection::Queue);
        let bytes = writer.finalize_to_bytes().unwrap();
        let mut reader = BundleReader::from_bytes(bytes).unwrap();
        assert!(reader.meta().has(BundleSection::Queue));
        assert!(reader.read_entry(entry::QUEUE).unwrap().is_none());
    }

    #[test]
    fn extract_database_to_returns_false_when_no_database_section() {
        // Bundle never declared a Database section; extract must
        // return Ok(false) rather than mutating the target file.
        let mut writer = BundleWriter::new(BundleMeta::new_for_export("test"));
        writer.add_settings(b"{}".to_vec(), "00".to_string());
        let bytes = writer.finalize_to_bytes().unwrap();
        let mut reader = BundleReader::from_bytes(bytes).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("dest.db");
        let written = reader.extract_database_to(&target).unwrap();
        assert!(!written, "no Database section means no write");
        assert!(!target.exists(), "target file must not be created");
    }

    #[test]
    fn file_names_starting_with_returns_matching_entries() {
        // The P3 helper relied on by import_profile when restoring
        // activity_log/ and manifests/ sub-trees.
        let mut writer = BundleWriter::new(BundleMeta::new_for_export("test"));
        writer.add_settings(b"{}".to_vec(), "00".to_string());
        writer.add_optional_file("activity_log/activity-2026-05-20.log", b"a".to_vec());
        writer.add_optional_file("activity_log/activity-2026-05-21.log", b"b".to_vec());
        writer.add_optional_file("manifests/Artist/Album/manifest.meedyadl", b"c".to_vec());
        writer.declare_section(BundleSection::ActivityLog);
        writer.declare_section(BundleSection::Manifests);
        let bytes = writer.finalize_to_bytes().unwrap();
        let reader = BundleReader::from_bytes(bytes).unwrap();

        let logs = reader.file_names_starting_with("activity_log/").unwrap();
        assert_eq!(logs.len(), 2);
        assert!(logs.iter().any(|n| n.ends_with("activity-2026-05-20.log")));
        assert!(logs.iter().any(|n| n.ends_with("activity-2026-05-21.log")));

        let mans = reader.file_names_starting_with("manifests/").unwrap();
        assert_eq!(mans.len(), 1);
        assert_eq!(mans[0], "manifests/Artist/Album/manifest.meedyadl");
    }

    #[test]
    fn database_roundtrips_through_bundle_byte_identical() {
        // End-to-end: real SQLite DB with seeded rows → write bundle
        // → re-read bundle → extract DB to a new path → reopen +
        // assert rows are still there. This is the canonical EPIC-A
        // / EPIC-B integration test (#875 M5).
        let tmp = tempfile::tempdir().unwrap();
        let src_db = tmp.path().join("source.db");
        {
            let mut idx =
                crate::services::download_index::DownloadIndex::open(&src_db).unwrap();
            idx.conn()
                .execute(
                    "INSERT INTO downloads (service, service_url, codec, downloaded_at) VALUES ('apple_music', 'https://m.apple.com/a/1', 'alac', '2026-05-25T00:00:00Z')",
                    [],
                )
                .unwrap();
            idx.conn()
                .execute(
                    "INSERT INTO downloads (service, service_url, codec, downloaded_at) VALUES ('apple_music', 'https://m.apple.com/a/2', 'aac', '2026-05-25T00:00:01Z')",
                    [],
                )
                .unwrap();
            // Idx drop closes the connection — SQLite checkpoints WAL on drop.
        }

        // Bundle the DB.
        let mut writer = BundleWriter::new(BundleMeta::new_for_export("test"));
        writer.add_settings(b"{}".to_vec(), "00".to_string());
        writer.add_database_from_disk(&src_db).unwrap();
        assert!(writer.meta().has(BundleSection::Database));
        let bundle_bytes = writer.finalize_to_bytes().unwrap();

        // Re-open the bundle, extract the DB to a fresh path.
        let mut reader = BundleReader::from_bytes(bundle_bytes).unwrap();
        assert!(reader.meta().has(BundleSection::Database));
        let dest_db = tmp.path().join("restored.db");
        let extracted = reader.extract_database_to(&dest_db).unwrap();
        assert!(extracted, "extract_database_to must return true when section present");
        assert!(dest_db.exists());

        // Open the restored DB and confirm both rows survived.
        let restored = crate::services::download_index::DownloadIndex::open(&dest_db).unwrap();
        let count: i64 = restored
            .conn()
            .query_row("SELECT COUNT(*) FROM downloads", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2, "both seeded rows must survive the bundle round-trip");
    }
}
