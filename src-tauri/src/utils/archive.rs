// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Archive download and extraction utilities.
// ============================================
//
// This module handles two closely related tasks:
//   1. **Downloading** files from HTTP(S) URLs with streaming I/O and
//      progress logging (no buffering the entire file in memory).
//   2. **Extracting** downloaded archives in ZIP or TAR.GZ format into
//      a destination directory on disk.
//
// These operations are used by:
//   - `services::python_manager` -- to download and unpack the portable
//     Python runtime from python-build-standalone GitHub releases.
//   - `services::dependency_manager` -- to download and unpack external
//     tool binaries (FFmpeg, mp4decrypt, etc.) from their release pages.
//
// Archive format selection:
//   - **ZIP** is used for Windows tool downloads (and some cross-platform
//     releases). Handled by the `zip` crate.
//   - **TAR.GZ** (gzip-compressed tar) is used for macOS/Linux downloads
//     and for python-build-standalone releases. Handled by the `flate2`
//     (gzip decompression) and `tar` (tar unpacking) crates.
//   - **TAR.XZ** (xz-compressed tar) is used by BtbN's Linux x86_64
//     FFmpeg builds (#981), which are published only in this format.
//     Handled by the `lzma-rs` (XZ decompression) and `tar` (tar
//     unpacking) crates.
//
// Threading model:
//   The `zip`, `flate2`, and `tar` crates are all synchronous (blocking)
//   I/O. Since this module runs inside a Tokio async runtime, blocking
//   extraction is offloaded to `tokio::task::spawn_blocking()` to avoid
//   starving the async executor.
//
// Reference: https://docs.rs/zip/latest/zip/
// Reference: https://docs.rs/tar/latest/tar/
// Reference: https://docs.rs/flate2/latest/flate2/
// Reference: https://docs.rs/reqwest/latest/reqwest/
// Reference: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html

use sha2::{Digest, Sha256};
use std::path::Path;
// `AsyncWriteExt` provides `.write_all()` and `.flush()` on Tokio's
// async `File` type, enabling non-blocking writes during download streaming.
// Reference: https://docs.rs/tokio/latest/tokio/io/trait.AsyncWriteExt.html
use tokio::io::AsyncWriteExt;

/// Supported archive formats for dependency downloads.
///
/// This enum is used by [`download_and_extract`] to select the correct
/// extraction strategy. The caller (typically a service module) determines
/// the format based on the download URL's file extension or the platform.
///
/// # Derive macros
/// - `Debug` -- enables `{:?}` formatting for log messages
/// - `Clone` -- allows the enum to be cheaply copied (it has no heap data)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveFormat {
    /// ZIP archive (commonly used for Windows tool downloads).
    /// Extracted by [`extract_zip`] using the `zip` crate.
    /// Reference: <https://docs.rs/zip/latest/zip>/
    Zip,
    /// TAR.GZ (gzip-compressed tar) archive.
    /// Used for macOS/Linux downloads and python-build-standalone releases.
    /// Extracted by [`extract_tar_gz`] using the `flate2` + `tar` crates.
    /// Reference: <https://docs.rs/flate2/latest/flate2>/
    /// Reference: <https://docs.rs/tar/latest/tar>/
    TarGz,
    /// TAR.XZ (xz-compressed tar) archive. Used by BtbN's Linux x86_64
    /// FFmpeg builds (#981), published only as `.tar.xz`. Extracted by
    /// [`extract_tar_xz`] using the pure-Rust `lzma-rs` + `tar` crates.
    TarXz,
}

/// Downloads a file from a URL to a local path using streaming I/O.
///
/// Writes chunks to disk as they arrive via `reqwest`'s `.chunk()` iterator
/// rather than buffering the entire response body in memory. This is critical
/// for large downloads (Python runtime ~70 MB, `FFmpeg` ~90 MB) where holding
/// the full payload in RAM would be wasteful.
///
/// Progress is logged at every 10% milestone using `log::info!`. The total
/// download size is determined from the HTTP `Content-Length` header; if the
/// server does not provide it, progress percentages are not logged.
///
/// Parent directories are created automatically if they do not exist.
///
/// # Arguments
/// * `url` - The HTTP(S) URL to download from. Redirects are followed
///   automatically by `reqwest`.
/// * `dest` - The local file path to write the downloaded content to.
///
/// # Errors
///
/// Returns `Err(String)` if the HTTP request fails, the response status is
/// non-success, or writing to the destination file fails.
///
/// # Returns
/// * `Ok((total_bytes, sha256_hex))` - The total bytes written and the
///   lowercase hex-encoded SHA-256 hash of the downloaded content.
/// * `Err(message)` - A human-readable error message if any step failed
///   (DNS resolution, HTTP error, I/O error, etc.).
///
/// # Reference
/// - `reqwest::get`: <https://docs.rs/reqwest/latest/reqwest/fn.get.html>
/// - `Response::chunk`: <https://docs.rs/reqwest/latest/reqwest/struct.Response.html#method.chunk>
/// - `tokio::fs::File`: <https://docs.rs/tokio/latest/tokio/fs/struct.File.html>
#[allow(clippy::cast_precision_loss)] // Byte-to-MB conversion for display; precision loss is negligible
pub async fn download_file(url: &str, dest: &Path) -> Result<(u64, String), String> {
    log::info!("Downloading: {} -> {}", url, dest.display());

    // Create parent directories if they don't exist
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    // Build an HTTP client with a connect timeout to prevent indefinite
    // stalls when the remote server is unreachable (e.g., DNS failure,
    // firewall). The 30-second connect timeout is generous enough for slow
    // networks but prevents blocking the dependency installation flow forever.
    // Note: no overall read timeout is set because large binary downloads
    // (FFmpeg ~90 MB, Python ~70 MB) legitimately take minutes on slow links;
    // the per-chunk streaming model below will surface mid-stream failures
    // promptly via the `.chunk()` error path.
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to start download from {url}: {e}"))?;

    // Check for HTTP errors (4xx, 5xx status codes)
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP error {status} downloading {url}"));
    }

    // Get total size for progress reporting (0 if server doesn't provide Content-Length)
    let total_size = response.content_length().unwrap_or(0);
    if total_size > 0 {
        log::info!("Download size: {:.1} MB", total_size as f64 / 1_048_576.0);
    }

    // Create the output file for streaming writes
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("Failed to create file {}: {}", dest.display(), e))?;

    // Stream the response body in chunks, writing each chunk to disk as
    // it arrives. `downloaded` tracks total bytes for progress calculation.
    // `last_logged_percent` prevents duplicate log lines by tracking the
    // last 10%-aligned milestone that was logged.
    // The SHA-256 hasher accumulates a digest across all chunks for
    // integrity verification after the download completes.
    let mut downloaded: u64 = 0;
    let mut last_logged_percent: u64 = 0;
    let mut hasher = Sha256::new();

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Failed to read download chunk: {e}"))?
    {
        // Feed each chunk into the SHA-256 hasher before writing to disk
        hasher.update(&chunk);

        // Write the received chunk to the output file
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write to {}: {}", dest.display(), e))?;

        downloaded += chunk.len() as u64;

        // Log progress at every 10% milestone. `checked_div` skips the
        // divide-by-zero case cleanly when `total_size` is not yet known.
        if let Some(percent) = downloaded.checked_mul(100).and_then(|n| n.checked_div(total_size)) {
            if percent >= last_logged_percent + 10 {
                log::info!(
                    "Download progress: {}% ({:.1}/{:.1} MB)",
                    percent,
                    downloaded as f64 / 1_048_576.0,
                    total_size as f64 / 1_048_576.0
                );
                last_logged_percent = percent;
            }
        }
    }

    // Flush the file to ensure all data is written to disk
    file.flush()
        .await
        .map_err(|e| format!("Failed to flush file {}: {}", dest.display(), e))?;

    // Finalize the SHA-256 hash and format as lowercase hex string
    let sha256_hex = format!("{:x}", hasher.finalize());

    log::info!(
        "Download complete: {:.1} MB (SHA-256: {})",
        downloaded as f64 / 1_048_576.0,
        sha256_hex
    );
    Ok((downloaded, sha256_hex))
}

/// Extracts a ZIP archive to the specified destination directory.
///
/// Iterates over every entry in the ZIP file and extracts it to `dest`.
/// Handles both directory entries (created via `create_dir_all`) and file
/// entries (extracted via `std::io::copy`). On Unix systems, file
/// permissions stored in the ZIP metadata (the "external attributes"
/// field) are preserved, which is important for executable binaries
/// like `FFmpeg` and mp4decrypt.
///
/// # Security
/// Uses `ZipFile::enclosed_name()` instead of `name()` to prevent
/// **zip-slip** path traversal attacks, where a malicious archive could
/// contain entries like `../../etc/passwd`. `enclosed_name()` returns
/// `None` for any path that would escape the destination directory.
///
/// # Threading
/// The `zip` crate performs synchronous (blocking) I/O, so the entire
/// extraction is wrapped in `tokio::task::spawn_blocking()` to avoid
/// blocking the Tokio async runtime's worker threads.
///
/// # Arguments
/// * `archive_path` - Path to the ZIP file to extract.
/// * `dest` - Directory to extract contents into (created if it doesn't exist).
///
/// # Errors
///
/// Returns `Err(String)` if the archive cannot be opened, read, or extracted
/// to the destination directory.
///
/// # Returns
/// * `Ok(())` on successful extraction.
/// * `Err(message)` if the archive cannot be opened, read, or extracted.
///
/// # Reference
/// - `ZipArchive::new`: <https://docs.rs/zip/latest/zip/read/struct.ZipArchive.html#method.new>
/// - `ZipFile::enclosed_name`: <https://docs.rs/zip/latest/zip/read/struct.ZipFile.html#method.enclosed_name>
/// - `spawn_blocking`: <https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html>
pub async fn extract_zip(archive_path: &Path, dest: &Path) -> Result<(), String> {
    log::info!(
        "Extracting ZIP: {} -> {}",
        archive_path.display(),
        dest.display()
    );

    // Create destination directory if it doesn't exist
    std::fs::create_dir_all(dest)
        .map_err(|e| format!("Failed to create directory {}: {}", dest.display(), e))?;

    // Clone `Path` references into owned `PathBuf` values because the
    // `spawn_blocking` closure must be `'static` (it may outlive the
    // current async function's borrows).
    let archive_path = archive_path.to_path_buf();
    let dest = dest.to_path_buf();

    // Offload synchronous ZIP extraction to Tokio's blocking thread pool.
    // `spawn_blocking` runs the closure on a dedicated OS thread so it
    // doesn't block the async task executor.
    tokio::task::spawn_blocking(move || {
        // Open the ZIP file for reading and parse its central directory.
        // The central directory (at the end of the file) contains metadata
        // for all entries, allowing random access by index.
        // Reference: https://docs.rs/zip/latest/zip/read/struct.ZipArchive.html
        let file = std::fs::File::open(&archive_path)
            .map_err(|e| format!("Failed to open ZIP file: {e}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP archive: {e}"))?;

        let total_entries = archive.len();
        log::info!("ZIP contains {total_entries} entries");

        // Extract each entry in the archive
        for i in 0..total_entries {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Failed to read ZIP entry {i}: {e}"))?;

            // Use `enclosed_name()` for security -- it validates that the
            // entry's path does not escape the extraction directory via `..`
            // components or absolute paths (zip-slip prevention).
            let outpath = if let Some(path) = entry.enclosed_name() {
                dest.join(path)
            } else {
                log::warn!("Skipping ZIP entry with unsafe path at index {i}");
                continue;
            };

            if entry.is_dir() {
                // Create directory entries
                std::fs::create_dir_all(&outpath).map_err(|e| {
                    format!("Failed to create directory {}: {}", outpath.display(), e)
                })?;
            } else {
                // Create parent directories for file entries
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        format!("Failed to create directory {}: {}", parent.display(), e)
                    })?;
                }

                // Extract the file content
                let mut outfile = std::fs::File::create(&outpath)
                    .map_err(|e| format!("Failed to create file {}: {}", outpath.display(), e))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| format!("Failed to extract {}: {}", outpath.display(), e))?;

                // Preserve Unix file permissions from the archive metadata.
                // This is critical for tool binaries (FFmpeg, mp4decrypt, etc.)
                // which need the execute bit set to run. The `#[cfg(unix)]`
                // attribute ensures this block is only compiled on macOS/Linux;
                // on Windows, executability is determined by file extension,
                // not permissions.
                //
                // `entry.unix_mode()` returns `Some(mode)` if the ZIP was
                // created on a Unix system and stored permission bits.
                // Windows-created ZIPs typically return `None`.
                //
                // `.ok()` discards any error (best-effort) -- failing to set
                // permissions is not worth aborting the entire extraction.
                //
                // Reference: https://doc.rust-lang.org/std/os/unix/fs/trait.PermissionsExt.html
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = entry.unix_mode() {
                        std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))
                            .ok();
                    }
                }
            }
        }

        log::info!("ZIP extraction complete: {total_entries} entries");
        Ok(())
    })
    .await
    .map_err(|e| format!("ZIP extraction task panicked: {e}"))?
}

/// Extracts a TAR.GZ archive to the specified destination directory.
///
/// TAR.GZ extraction is a two-layer process:
///   1. **Gzip decompression** -- the `flate2::read::GzDecoder` wraps the
///      file reader and transparently decompresses the gzip stream.
///   2. **Tar unpacking** -- the `tar::Archive` reads the decompressed tar
///      stream and extracts all entries (files, directories, symlinks)
///      to the destination directory.
///
/// File permissions and ownership metadata are automatically preserved by
/// the `tar` crate on Unix systems (via `set_preserve_permissions(true)`).
///
/// # Threading
/// Like [`extract_zip`], the `flate2` and `tar` crates perform synchronous
/// I/O, so extraction is wrapped in `tokio::task::spawn_blocking()`.
///
/// # Arguments
/// * `archive_path` - Path to the `.tar.gz` file to extract.
/// * `dest` - Directory to extract contents into (created if it doesn't exist).
///
/// # Errors
///
/// Returns `Err(String)` if the archive cannot be opened, decompressed,
/// or unpacked to the destination directory.
///
/// # Returns
/// * `Ok(())` on successful extraction.
/// * `Err(message)` if the archive cannot be opened, decompressed, or unpacked.
///
/// # Security
/// Iterates entries individually and validates each path to prevent
/// **tar-slip** path traversal attacks, where a malicious archive
/// could contain entries like `../../etc/crontab`. Entries with `..`
/// components or absolute paths are skipped with a warning.
///
/// # Reference
/// - `GzDecoder`: <https://docs.rs/flate2/latest/flate2/read/struct.GzDecoder.html>
/// - `Archive::entries`: <https://docs.rs/tar/latest/tar/struct.Archive.html#method.entries>
/// - `spawn_blocking`: <https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html>
pub async fn extract_tar_gz(archive_path: &Path, dest: &Path) -> Result<(), String> {
    log::info!(
        "Extracting TAR.GZ: {} -> {}",
        archive_path.display(),
        dest.display()
    );

    // Create destination directory if it doesn't exist
    std::fs::create_dir_all(dest)
        .map_err(|e| format!("Failed to create directory {}: {}", dest.display(), e))?;

    // Clone paths for the blocking closure
    let archive_path = archive_path.to_path_buf();
    let dest = dest.to_path_buf();

    // Run synchronous extraction in a blocking thread pool task
    tokio::task::spawn_blocking(move || {
        // Open the .tar.gz file and create a layered reader:
        //   File -> GzDecoder (decompresses gzip) -> Archive (reads tar)
        // This streaming pipeline avoids writing an intermediate
        // decompressed tar file to disk.
        let file = std::fs::File::open(&archive_path)
            .map_err(|e| format!("Failed to open archive {}: {}", archive_path.display(), e))?;
        let decoder = flate2::read::GzDecoder::new(file);
        unpack_tar_stream(decoder, &dest)?;

        log::info!("TAR.GZ extraction complete to {}", dest.display());
        Ok(())
    })
    .await
    .map_err(|e| format!("TAR.GZ extraction task panicked: {e}"))?
}

/// Shared tar-entry iteration core used by both [`extract_tar_gz`] and
/// [`extract_tar_xz`].
///
/// Takes any decompressed tar byte stream (`R: Read`) — a `GzDecoder` for
/// TAR.GZ, or a plain `File` reading an already-XZ-decoded temp tar for
/// TAR.XZ — and unpacks its entries into `dest`, applying the same
/// tar-slip and symlink/hardlink defences on every call site.
///
/// # Security
/// Iterates entries individually and validates each path to prevent
/// **tar-slip** path traversal attacks, where a malicious archive
/// could contain entries like `../../etc/crontab`. Entries with `..`
/// components or absolute paths are skipped with a warning. Symlink
/// and hardlink entries are rejected outright (see inline comment).
///
/// # Errors
///
/// Returns `Err(String)` if the tar stream cannot be read or an entry
/// cannot be unpacked to the destination directory.
fn unpack_tar_stream<R: std::io::Read>(reader: R, dest: &Path) -> Result<(), String> {
    let mut archive = tar::Archive::new(reader);

    // Iterate entries individually for path traversal validation,
    // rather than using `archive.unpack()` which extracts blindly.
    let entries = archive
        .entries()
        .map_err(|e| format!("Failed to read tar entries: {e}"))?;

    for entry_result in entries {
        let mut entry = entry_result.map_err(|e| format!("Failed to read tar entry: {e}"))?;

        let entry_path = entry
            .path()
            .map_err(|e| format!("Failed to read tar entry path: {e}"))?
            .into_owned();

        // Security: reject entries with ".." components or absolute
        // paths that could escape the destination directory (tar-slip).
        if entry_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
            || entry_path.is_absolute()
        {
            log::warn!(
                "Skipping tar entry with unsafe path: {}",
                entry_path.display()
            );
            continue;
        }

        // Security: reject symlink / hardlink entries. This loop uses
        // `entry.unpack()` (below), which does NOT confine a link's
        // target — a malicious archive could plant a symlink pointing
        // outside `dest`, then a subsequent regular-file entry would
        // write *through* that link to escape the extraction directory
        // (tar-slip via symlink, which the `..`/absolute check above
        // does not catch). MeedyaDL only extracts tool-binary archives,
        // which never legitimately contain links, so rejecting them
        // outright is safe and closes the vector.
        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            log::warn!(
                "Skipping tar symlink/hardlink entry (not permitted in tool archives): {}",
                entry_path.display()
            );
            continue;
        }

        let outpath = dest.join(&entry_path);

        // Create parent directories as needed
        if let Some(parent) = outpath.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {e}", parent.display()))?;
        }

        // Extract the entry, preserving permissions
        entry
            .unpack(&outpath)
            .map_err(|e| format!("Failed to extract tar entry {}: {e}", entry_path.display()))?;
    }

    Ok(())
}

/// Extracts a TAR.XZ archive to the specified destination directory.
///
/// TAR.XZ extraction is a two-step process, unlike TAR.GZ's single-pass
/// streaming pipeline:
///   1. **XZ decompression** -- `lzma_rs::xz_decompress()` reads the whole
///      XZ stream and writes the decompressed tar bytes to a sibling temp
///      file (`{archive}.decoded.tar`). Unlike `flate2::GzDecoder`,
///      `lzma-rs`'s XZ decoder is not a `Read` adapter that can be layered
///      directly in front of `tar::Archive` -- it operates on whole
///      reader/writer pairs -- so a temp file is the simplest correct
///      bridge between the two crates.
///   2. **Tar unpacking** -- the decompressed temp file is reopened and
///      handed to the same [`unpack_tar_stream`] core used by
///      [`extract_tar_gz`], so both formats share identical tar-slip and
///      symlink/hardlink defences.
///
/// `lzma-rs` supports LZMA2 payloads with CRC32/CRC64/SHA256/None
/// integrity checks (the common case for real-world `.tar.xz` releases)
/// but does not implement XZ's optional BCJ (branch/call/jump) filters --
/// none of MeedyaDL's known `.tar.xz` sources (BtbN FFmpeg builds) use them.
///
/// The temp tar file is always cleaned up (best-effort) regardless of
/// whether unpacking succeeded, since every failure path here falls
/// through to the caller (and ultimately the mirror fallback in
/// `services::dependency_manager`).
///
/// # Threading
/// Like [`extract_tar_gz`], both the XZ decompression and tar unpacking
/// are synchronous (blocking) operations, so the entire function body is
/// wrapped in `tokio::task::spawn_blocking()`.
///
/// # Arguments
/// * `archive_path` - Path to the `.tar.xz` file to extract.
/// * `dest` - Directory to extract contents into (created if it doesn't exist).
///
/// # Errors
///
/// Returns `Err(String)` if the archive cannot be opened, decompressed,
/// or unpacked to the destination directory.
///
/// # Returns
/// * `Ok(())` on successful extraction.
/// * `Err(message)` if the archive cannot be opened, decompressed, or unpacked.
///
/// # Reference
/// - `lzma_rs::xz_decompress`: <https://docs.rs/lzma-rs/latest/lzma_rs/fn.xz_decompress.html>
/// - `Archive::entries`: <https://docs.rs/tar/latest/tar/struct.Archive.html#method.entries>
/// - `spawn_blocking`: <https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html>
pub async fn extract_tar_xz(archive_path: &Path, dest: &Path) -> Result<(), String> {
    log::info!(
        "Extracting TAR.XZ: {} -> {}",
        archive_path.display(),
        dest.display()
    );

    // Create destination directory if it doesn't exist
    std::fs::create_dir_all(dest)
        .map_err(|e| format!("Failed to create directory {}: {}", dest.display(), e))?;

    // Clone paths for the blocking closure
    let archive_path = archive_path.to_path_buf();
    let dest = dest.to_path_buf();

    // Run synchronous decompression + extraction in a blocking thread pool task
    tokio::task::spawn_blocking(move || {
        // Sibling temp file to hold the decompressed tar stream. Named
        // `{archive}.decoded.tar` so it lives alongside the source
        // archive rather than inside `dest`.
        let decoded_tar_path = archive_path.with_extension("decoded.tar");

        // Step 1: XZ-decompress the whole archive into the temp tar file.
        let decompress_result = (|| -> Result<(), String> {
            let input_file = std::fs::File::open(&archive_path).map_err(|e| {
                format!("Failed to open archive {}: {}", archive_path.display(), e)
            })?;
            let mut reader = std::io::BufReader::new(input_file);

            let output_file = std::fs::File::create(&decoded_tar_path).map_err(|e| {
                format!(
                    "Failed to create temp tar file {}: {}",
                    decoded_tar_path.display(),
                    e
                )
            })?;
            let mut writer = std::io::BufWriter::new(output_file);

            lzma_rs::xz_decompress(&mut reader, &mut writer)
                .map_err(|e| format!("Failed to decompress XZ stream: {e}"))?;

            // Flush explicitly (in addition to the implicit flush-on-drop)
            // so any I/O error surfaces here rather than being silently
            // swallowed when `writer` goes out of scope.
            use std::io::Write;
            writer
                .flush()
                .map_err(|e| format!("Failed to flush decompressed tar file: {e}"))?;
            drop(writer);

            Ok(())
        })();

        // Step 2: unpack the decompressed tar, sharing the same tar-slip
        // and symlink/hardlink defences as TAR.GZ. Only attempted if
        // decompression succeeded.
        let unpack_result = decompress_result.and_then(|()| {
            let tar_file = std::fs::File::open(&decoded_tar_path).map_err(|e| {
                format!(
                    "Failed to reopen decompressed tar file {}: {}",
                    decoded_tar_path.display(),
                    e
                )
            })?;
            unpack_tar_stream(tar_file, &dest)
        });

        // Step 3: best-effort cleanup of the temp tar file, in ALL cases
        // (success or failure) — never leave a multi-hundred-MB temp file
        // behind on disk.
        let _ = std::fs::remove_file(&decoded_tar_path);

        unpack_result?;

        log::info!("TAR.XZ extraction complete to {}", dest.display());
        Ok(())
    })
    .await
    .map_err(|e| format!("TAR.XZ extraction task panicked: {e}"))?
}

/// Detects the archive format from a URL's file extension.
///
/// This is an **honest** detection function: it returns `None` for any
/// URL whose extension it doesn't recognize, rather than silently
/// guessing a format (the pre-#981 mirror-resolution code defaulted
/// every non-`.zip` extension to `TarGz`, which mislabeled `.tar.xz`
/// assets and caused 100% primary-extract failure for BtbN's Linux
/// x86_64 FFmpeg build). Callers should treat `None` as "format unknown
/// -- fall back to a caller-chosen default and log a warning", never as
/// an implicit "assume gzip".
///
/// Recognizes (case-insensitively): `.zip`, `.tar.xz` / `.txz`,
/// `.tar.gz` / `.tgz`.
///
/// Note: some legitimate upstream URLs have no file extension at all
/// (e.g. evermeet.cx's FFmpeg endpoint `https://evermeet.cx/ffmpeg/getrelease/zip`,
/// where `zip` is a path segment, not a `.zip` suffix). Such URLs return
/// `None` by design -- the caller already knows the format out-of-band
/// for hardcoded URLs like that one, so there's no format ambiguity to
/// resolve here in the first place.
///
/// # Arguments
/// * `url` - The download URL (or bare filename) to inspect.
///
/// # Returns
/// * `Some(format)` if the extension is recognized.
/// * `None` if the extension is missing or unrecognized.
#[must_use]
pub fn detect_archive_format_from_url(url: &str) -> Option<ArchiveFormat> {
    let lower = url.to_lowercase();
    if lower.ends_with(".zip") {
        Some(ArchiveFormat::Zip)
    } else if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
        Some(ArchiveFormat::TarXz)
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        Some(ArchiveFormat::TarGz)
    } else {
        None
    }
}

/// Downloads a file from a URL and extracts it to the destination directory.
///
/// This is the **primary entry point** for installing dependencies. It
/// orchestrates the complete download-and-extract pipeline:
///
/// 1. **Download** -- streams the archive from the URL to a temporary file
///    in `{system_temp}/meedyadl-downloads/`. Using a dedicated temp
///    subdirectory avoids naming conflicts with other applications.
/// 2. **Extract** -- delegates to the appropriate extractor based on `format`:
///    - `ArchiveFormat::Zip` -> [`extract_zip`]
///    - `ArchiveFormat::TarGz` -> [`extract_tar_gz`]
///    - `ArchiveFormat::TarXz` -> [`extract_tar_xz`]
/// 3. **Cleanup** -- deletes the temporary download file (best-effort; a
///    failure to delete is logged as a warning but does not fail the operation).
///
/// # Arguments
/// * `url` - The HTTP(S) URL to download the archive from.
/// * `dest` - The directory to extract the archive contents into.
/// * `format` - The expected archive format ([`ArchiveFormat::Zip`],
///   [`ArchiveFormat::TarGz`], or [`ArchiveFormat::TarXz`]).
///
/// # Errors
///
/// Returns `Err(String)` if the download, checksum verification, or
/// extraction step fails.
///
/// # Returns
/// * `Ok(())` if download, verification, and extraction all succeeded.
/// * `Err(message)` if any step failed.
///
/// # Connection
/// Called by `services::python_manager::install_python()` and
/// `services::dependency_manager::install_dependency()`.
pub async fn download_and_extract(
    url: &str,
    dest: &Path,
    format: ArchiveFormat,
) -> Result<(), String> {
    download_and_extract_verified(url, dest, format, None).await
}

/// Downloads a file and extracts it, optionally verifying a SHA-256 checksum.
///
/// This is the verified variant of [`download_and_extract`]. When
/// `expected_sha256` is provided, the computed hash of the downloaded file
/// is compared against it. A mismatch deletes the temp file and returns
/// an error before extraction, preventing use of corrupted or tampered archives.
///
/// # Arguments
/// * `url` - The HTTP(S) URL to download the archive from.
/// * `dest` - The directory to extract the archive contents into.
/// * `format` - The expected archive format.
/// * `expected_sha256` - If `Some`, the expected lowercase hex SHA-256 hash.
///   If `None`, the hash is computed and logged but not verified.
pub async fn download_and_extract_verified(
    url: &str,
    dest: &Path,
    format: ArchiveFormat,
    expected_sha256: Option<&str>,
) -> Result<(), String> {
    // Derive a temp file name from the last path segment of the URL.
    // For example, "https://github.com/.../python-3.12.tar.gz" yields
    // "python-3.12.tar.gz". Falls back to "download.tmp" if the URL
    // has no path segments (unlikely for real download URLs).
    let file_name = url.rsplit('/').next().unwrap_or("download.tmp");

    // Use a MeedyaDL-specific temp directory to avoid conflicts
    let temp_dir = std::env::temp_dir().join("meedyadl-downloads");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {e}"))?;
    let temp_file = temp_dir.join(file_name);

    // Step 1: Download the archive to the temp file.
    // On failure, clean up the partial temp file before propagating the error
    // to avoid leaving large (potentially hundreds of MB) stale files on disk.
    let (_bytes, sha256) = match download_file(url, &temp_file).await {
        Ok(result) => result,
        Err(e) => {
            let _ = tokio::fs::remove_file(&temp_file).await;
            return Err(e);
        }
    };

    // Step 1b: Verify SHA-256 checksum if an expected hash was provided.
    // This catches corrupted downloads and supply-chain tampering before
    // the archive is extracted (and potentially executed as tool binaries).
    if let Some(expected) = expected_sha256 {
        if sha256 != expected {
            let _ = tokio::fs::remove_file(&temp_file).await;
            return Err(format!(
                "SHA-256 checksum mismatch for {url}\n  Expected: {expected}\n  Actual:   {sha256}"
            ));
        }
        log::info!("SHA-256 checksum verified for {url}");
    }

    // Step 2: Extract the archive to the destination
    let result = match format {
        ArchiveFormat::Zip => extract_zip(&temp_file, dest).await,
        ArchiveFormat::TarGz => extract_tar_gz(&temp_file, dest).await,
        ArchiveFormat::TarXz => extract_tar_xz(&temp_file, dest).await,
    };

    // Step 3: Clean up the temporary file (best-effort)
    if let Err(e) = tokio::fs::remove_file(&temp_file).await {
        log::warn!(
            "Failed to clean up temp file {}: {}",
            temp_file.display(),
            e
        );
    }

    result
}

/// Sets executable permissions on a file (Unix only).
///
/// On Unix systems, adds the execute bit for owner, group, and others
/// (`chmod a+x`). This is equivalent to `current_mode | 0o111`. On
/// Windows this function is a no-op because executability is determined
/// by file extension (`.exe`, `.bat`, `.cmd`), not by permission bits.
///
/// # Arguments
/// * `path` - Path to the file to mark as executable.
///
/// # Returns
/// * `Ok(())` on success or on Windows (no-op).
/// * `Err(message)` if reading metadata or setting permissions fails on Unix.
///
/// # Why `#[allow(unused_variables)]`?
/// On Windows, the `path` parameter is not used inside the function body
/// (the `#[cfg(unix)]` block is compiled out). Without this attribute, the
/// compiler would emit an "unused variable" warning on Windows builds.
///
/// # Connection
/// Called by `services::dependency_manager` after extracting tool binaries
/// on macOS/Linux to ensure they can be executed as subprocesses.
///
/// # Errors
///
/// Returns `Err(String)` if reading file metadata or setting permissions fails.
///
/// # Reference
/// - `PermissionsExt`: <https://doc.rust-lang.org/std/os/unix/fs/trait.PermissionsExt.html>
/// - `fs::set_permissions`: <https://doc.rust-lang.org/std/fs/fn.set_permissions.html>
#[allow(unused_variables)]
pub fn set_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Read the file's current metadata to get its existing permission mode.
        let metadata = std::fs::metadata(path)
            .map_err(|e| format!("Failed to read metadata for {}: {}", path.display(), e))?;
        let mut perms = metadata.permissions();
        // Bitwise OR with 0o111 adds the execute bit for user (0o100),
        // group (0o010), and others (0o001) while preserving all existing
        // permission bits (read, write, setuid, etc.).
        let mode = perms.mode() | 0o111;
        perms.set_mode(mode);
        std::fs::set_permissions(path, perms)
            .map_err(|e| format!("Failed to set permissions on {}: {}", path.display(), e))?;
        log::debug!("Set executable permission on {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real, minimal `.tar.xz` archive (208 bytes) generated for these
    /// tests. Contains a single entry, `hello.txt`, whose content is the
    /// literal string `"MeedyaDL tar.xz extraction test\n"`.
    const XZ_FIXTURE: &[u8] = &[
        0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0x00, 0x04, 0xE6, 0xD6, 0xB4, 0x46,
        0x02, 0x00, 0x21, 0x01, 0x1C, 0x00, 0x00, 0x00, 0x10, 0xCF, 0x58, 0xCC,
        0xE0, 0x27, 0xFF, 0x00, 0x8E, 0x5D, 0x00, 0x34, 0x19, 0x49, 0xEE, 0x8D,
        0xF0, 0xBA, 0xC8, 0xFF, 0x9B, 0xFF, 0xF2, 0x0C, 0x69, 0xAF, 0x11, 0xEB,
        0x63, 0x54, 0x89, 0x1D, 0xF7, 0x2E, 0x76, 0x2F, 0x4B, 0x50, 0x8B, 0x79,
        0x9B, 0x58, 0x59, 0xCB, 0x18, 0x77, 0xBC, 0xFD, 0x72, 0x0F, 0xA2, 0xEA,
        0x0F, 0xF6, 0x68, 0x8A, 0x64, 0x0E, 0x0F, 0x55, 0x72, 0xEE, 0x13, 0xF7,
        0x04, 0xF8, 0x14, 0xFB, 0xCF, 0xF8, 0x17, 0x13, 0x5E, 0x3E, 0x2A, 0x80,
        0x4E, 0x52, 0x72, 0xCD, 0x4F, 0xC2, 0x69, 0x4B, 0xB7, 0xFF, 0x19, 0xC9,
        0xF9, 0x11, 0xAC, 0x40, 0xA0, 0xF8, 0xA6, 0xEA, 0x38, 0x72, 0xB8, 0xB7,
        0x6D, 0x93, 0x9C, 0x81, 0x84, 0xE0, 0x4C, 0xC9, 0x74, 0xC9, 0xD2, 0x1A,
        0xA8, 0x9B, 0x03, 0x98, 0xFC, 0x4E, 0x14, 0x67, 0x06, 0x5D, 0x39, 0x06,
        0x53, 0xDD, 0x0E, 0x5D, 0x07, 0xE0, 0xA4, 0x48, 0x4E, 0xBA, 0xCD, 0x73,
        0x3C, 0xCE, 0x47, 0xA8, 0x4F, 0x6D, 0x21, 0x2E, 0x70, 0x60, 0x2E, 0xD6,
        0xCD, 0xED, 0x8A, 0x83, 0x00, 0x00, 0x00, 0x00, 0xFD, 0xEA, 0x25, 0xB9,
        0x4D, 0xFA, 0x97, 0xF4, 0x00, 0x01, 0xAA, 0x01, 0x80, 0x50, 0x00, 0x00,
        0x6E, 0x57, 0x57, 0xC7, 0xB1, 0xC4, 0x67, 0xFB, 0x02, 0x00, 0x00, 0x00,
        0x00, 0x04, 0x59, 0x5A,
    ];

    /// `detect_archive_format_from_url()` must recognise every extension
    /// MeedyaDL's tool sources actually use, case-insensitively, and must
    /// return `None` -- not a silent guess -- for anything else. This is
    /// the regression guard for the pre-#981 "assume gzip" default that
    /// mislabeled `.tar.xz` mirror/N_m3u8DL-RE assets.
    #[test]
    fn detect_archive_format_maps_known_extensions() {
        assert_eq!(
            detect_archive_format_from_url("https://example.com/tool.zip"),
            Some(ArchiveFormat::Zip)
        );
        assert_eq!(
            detect_archive_format_from_url("https://example.com/TOOL.ZIP"),
            Some(ArchiveFormat::Zip)
        );
        assert_eq!(
            detect_archive_format_from_url(
                "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz"
            ),
            Some(ArchiveFormat::TarXz)
        );
        assert_eq!(
            detect_archive_format_from_url("tool.txz"),
            Some(ArchiveFormat::TarXz)
        );
        assert_eq!(
            detect_archive_format_from_url("TOOL.TAR.XZ"),
            Some(ArchiveFormat::TarXz)
        );
        assert_eq!(
            detect_archive_format_from_url("https://example.com/tool.tar.gz"),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(
            detect_archive_format_from_url("tool.tgz"),
            Some(ArchiveFormat::TarGz)
        );
        // Extension-less URLs (e.g. evermeet.cx's FFmpeg endpoint, where
        // "zip" is a path segment, not a `.zip` suffix) must return None
        // rather than silently guessing a format.
        assert_eq!(
            detect_archive_format_from_url("https://evermeet.cx/ffmpeg/getrelease/zip"),
            None
        );
        assert_eq!(detect_archive_format_from_url("tool.exe"), None);
    }

    /// End-to-end extraction of a real `.tar.xz` fixture: unpacks
    /// `hello.txt` with the expected content, and confirms the
    /// intermediate `{archive}.decoded.tar` temp file is cleaned up
    /// afterwards (it must not accumulate on disk across installs).
    #[tokio::test]
    async fn extract_tar_xz_unpacks_fixture() {
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("fixture.tar.xz");
        std::fs::write(&archive_path, XZ_FIXTURE).unwrap();

        let dest = temp_dir.path().join("extracted");

        extract_tar_xz(&archive_path, &dest).await.unwrap();

        let extracted_content = std::fs::read_to_string(dest.join("hello.txt")).unwrap();
        assert_eq!(extracted_content, "MeedyaDL tar.xz extraction test\n");

        // The sibling decoded-tar temp file must be cleaned up, not left
        // behind next to the source archive.
        let decoded_tar_path = archive_path.with_extension("decoded.tar");
        assert!(
            !decoded_tar_path.exists(),
            "decoded tar temp file was not cleaned up: {}",
            decoded_tar_path.display()
        );
    }

    /// Regression guard for the pre-#981 mislabel: feeding real XZ bytes
    /// into `extract_tar_gz()` (which assumes gzip) must fail cleanly
    /// rather than silently producing garbage or panicking.
    #[tokio::test]
    async fn extract_tar_gz_rejects_xz_bytes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("fixture.tar.xz");
        std::fs::write(&archive_path, XZ_FIXTURE).unwrap();

        let dest = temp_dir.path().join("extracted");

        let result = extract_tar_gz(&archive_path, &dest).await;
        assert!(
            result.is_err(),
            "extract_tar_gz() should reject XZ-compressed input, not silently succeed"
        );
    }
}
