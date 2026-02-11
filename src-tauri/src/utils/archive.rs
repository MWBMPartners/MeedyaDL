// Copyright (c) 2024-2026 MWBM Partners Ltd
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
#[derive(Debug, Clone)]
pub enum ArchiveFormat {
    /// ZIP archive (commonly used for Windows tool downloads).
    /// Extracted by [`extract_zip`] using the `zip` crate.
    /// Reference: https://docs.rs/zip/latest/zip/
    Zip,
    /// TAR.GZ (gzip-compressed tar) archive.
    /// Used for macOS/Linux downloads and python-build-standalone releases.
    /// Extracted by [`extract_tar_gz`] using the `flate2` + `tar` crates.
    /// Reference: https://docs.rs/flate2/latest/flate2/
    /// Reference: https://docs.rs/tar/latest/tar/
    TarGz,
}

/// Downloads a file from a URL to a local path using streaming I/O.
///
/// Writes chunks to disk as they arrive via `reqwest`'s `.chunk()` iterator
/// rather than buffering the entire response body in memory. This is critical
/// for large downloads (Python runtime ~70 MB, FFmpeg ~90 MB) where holding
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
/// # Returns
/// * `Ok(total_bytes)` - The total number of bytes written to disk.
/// * `Err(message)` - A human-readable error message if any step failed
///   (DNS resolution, HTTP error, I/O error, etc.).
///
/// # Reference
/// - `reqwest::get`: <https://docs.rs/reqwest/latest/reqwest/fn.get.html>
/// - `Response::chunk`: <https://docs.rs/reqwest/latest/reqwest/struct.Response.html#method.chunk>
/// - `tokio::fs::File`: <https://docs.rs/tokio/latest/tokio/fs/struct.File.html>
pub async fn download_file(url: &str, dest: &Path) -> Result<u64, String> {
    log::info!("Downloading: {} -> {}", url, dest.display());

    // Create parent directories if they don't exist
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    // Send an HTTP GET request using the default reqwest client.
    // `reqwest::get()` creates a one-shot client, follows redirects (up to
    // 10 by default), and returns the response with the body not yet
    // consumed. The `mut` is needed because `.chunk()` below advances
    // through the response body.
    let mut response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to start download from {}: {}", url, e))?;

    // Check for HTTP errors (4xx, 5xx status codes)
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP error {} downloading {}", status, url));
    }

    // Get total size for progress reporting (0 if server doesn't provide Content-Length)
    let total_size = response.content_length().unwrap_or(0);
    if total_size > 0 {
        log::info!(
            "Download size: {:.1} MB",
            total_size as f64 / 1_048_576.0
        );
    }

    // Create the output file for streaming writes
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("Failed to create file {}: {}", dest.display(), e))?;

    // Stream the response body in chunks, writing each chunk to disk as
    // it arrives. `downloaded` tracks total bytes for progress calculation.
    // `last_logged_percent` prevents duplicate log lines by tracking the
    // last 10%-aligned milestone that was logged.
    let mut downloaded: u64 = 0;
    let mut last_logged_percent: u64 = 0;

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Failed to read download chunk: {}", e))?
    {
        // Write the received chunk to the output file
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write to {}: {}", dest.display(), e))?;

        downloaded += chunk.len() as u64;

        // Log progress at every 10% milestone
        if total_size > 0 {
            let percent = (downloaded * 100) / total_size;
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

    log::info!("Download complete: {:.1} MB", downloaded as f64 / 1_048_576.0);
    Ok(downloaded)
}

/// Extracts a ZIP archive to the specified destination directory.
///
/// Iterates over every entry in the ZIP file and extracts it to `dest`.
/// Handles both directory entries (created via `create_dir_all`) and file
/// entries (extracted via `std::io::copy`). On Unix systems, file
/// permissions stored in the ZIP metadata (the "external attributes"
/// field) are preserved, which is important for executable binaries
/// like FFmpeg and mp4decrypt.
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
            .map_err(|e| format!("Failed to open ZIP file: {}", e))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP archive: {}", e))?;

        let total_entries = archive.len();
        log::info!("ZIP contains {} entries", total_entries);

        // Extract each entry in the archive
        for i in 0..total_entries {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;

            // Use `enclosed_name()` for security -- it validates that the
            // entry's path does not escape the extraction directory via `..`
            // components or absolute paths (zip-slip prevention).
            let outpath = match entry.enclosed_name() {
                Some(path) => dest.join(path),
                None => {
                    log::warn!("Skipping ZIP entry with unsafe path at index {}", i);
                    continue;
                }
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
                        std::fs::set_permissions(
                            &outpath,
                            std::fs::Permissions::from_mode(mode),
                        )
                        .ok();
                    }
                }
            }
        }

        log::info!("ZIP extraction complete: {} entries", total_entries);
        Ok(())
    })
    .await
    .map_err(|e| format!("ZIP extraction task panicked: {}", e))?
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
/// # Returns
/// * `Ok(())` on successful extraction.
/// * `Err(message)` if the archive cannot be opened, decompressed, or unpacked.
///
/// # Reference
/// - `GzDecoder`: <https://docs.rs/flate2/latest/flate2/read/struct.GzDecoder.html>
/// - `Archive::unpack`: <https://docs.rs/tar/latest/tar/struct.Archive.html#method.unpack>
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
        // `GzDecoder` implements `Read` and transparently decompresses the
        // gzip stream on each read call. It detects the gzip header
        // automatically.
        // Reference: https://docs.rs/flate2/latest/flate2/read/struct.GzDecoder.html
        let decoder = flate2::read::GzDecoder::new(file);
        // `Archive::new` wraps any `Read` implementor and interprets the
        // byte stream as a POSIX tar archive.
        // Reference: https://docs.rs/tar/latest/tar/struct.Archive.html
        let mut archive = tar::Archive::new(decoder);

        // Preserve Unix file permissions (mode bits) when extracting.
        // Without this, all files would get default permissions (0o644).
        archive.set_preserve_permissions(true);
        // Allow overwriting existing files at the destination. This ensures
        // re-installation of a dependency replaces old files cleanly.
        archive.set_overwrite(true);

        // Unpack all entries (files, directories, symlinks) to the
        // destination directory. The `tar` crate handles creation of
        // subdirectories and permission setting automatically.
        archive
            .unpack(&dest)
            .map_err(|e| format!("Failed to extract tar.gz archive: {}", e))?;

        log::info!("TAR.GZ extraction complete to {}", dest.display());
        Ok(())
    })
    .await
    .map_err(|e| format!("TAR.GZ extraction task panicked: {}", e))?
}

/// Downloads a file from a URL and extracts it to the destination directory.
///
/// This is the **primary entry point** for installing dependencies. It
/// orchestrates the complete download-and-extract pipeline:
///
/// 1. **Download** -- streams the archive from the URL to a temporary file
///    under `{system_temp}/gamdl-gui-downloads/`. Using a dedicated temp
///    subdirectory avoids naming conflicts with other applications.
/// 2. **Extract** -- delegates to the appropriate extractor based on `format`:
///    - `ArchiveFormat::Zip` -> [`extract_zip`]
///    - `ArchiveFormat::TarGz` -> [`extract_tar_gz`]
/// 3. **Cleanup** -- deletes the temporary download file (best-effort; a
///    failure to delete is logged as a warning but does not fail the operation).
///
/// # Arguments
/// * `url` - The HTTP(S) URL to download the archive from.
/// * `dest` - The directory to extract the archive contents into.
/// * `format` - The expected archive format ([`ArchiveFormat::Zip`] or
///   [`ArchiveFormat::TarGz`]).
///
/// # Returns
/// * `Ok(())` if both download and extraction succeeded.
/// * `Err(message)` if either step failed.
///
/// # Connection
/// Called by `services::python_manager::install_python()` and
/// `services::dependency_manager::install_dependency()`.
pub async fn download_and_extract(
    url: &str,
    dest: &Path,
    format: ArchiveFormat,
) -> Result<(), String> {
    // Derive a temp file name from the last path segment of the URL.
    // For example, "https://github.com/.../python-3.12.tar.gz" yields
    // "python-3.12.tar.gz". Falls back to "download.tmp" if the URL
    // has no path segments (unlikely for real download URLs).
    let file_name = url.rsplit('/').next().unwrap_or("download.tmp");

    // Use a gamdl-gui-specific temp directory to avoid conflicts
    let temp_dir = std::env::temp_dir().join("gamdl-gui-downloads");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;
    let temp_file = temp_dir.join(file_name);

    // Step 1: Download the archive to the temp file
    download_file(url, &temp_file).await?;

    // Step 2: Extract the archive to the destination
    let result = match format {
        ArchiveFormat::Zip => extract_zip(&temp_file, dest).await,
        ArchiveFormat::TarGz => extract_tar_gz(&temp_file, dest).await,
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
