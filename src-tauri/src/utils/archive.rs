// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Archive download and extraction utilities.
// Handles downloading files from URLs with streaming progress, and extracting
// ZIP and TAR.GZ archives. Used by the Python manager and dependency manager
// to install portable runtimes and tools from their GitHub release archives.

use std::path::Path;
use tokio::io::AsyncWriteExt;

/// Supported archive formats for dependency downloads.
#[derive(Debug, Clone)]
pub enum ArchiveFormat {
    /// ZIP archive (commonly used for Windows tool downloads)
    Zip,
    /// TAR.GZ (gzip-compressed tar) archive (macOS/Linux downloads, python-build-standalone)
    TarGz,
}

/// Downloads a file from a URL to a local path using streaming.
///
/// Writes chunks to disk as they arrive rather than buffering the entire
/// file in memory. Logs download progress at 10% intervals. Creates
/// parent directories automatically if they don't exist.
///
/// # Arguments
/// * `url` - The URL to download from
/// * `dest` - The local file path to write the downloaded content to
///
/// # Returns
/// * `Ok(total_bytes)` - The total number of bytes downloaded
/// * `Err(message)` - A descriptive error if the download failed
pub async fn download_file(url: &str, dest: &Path) -> Result<u64, String> {
    log::info!("Downloading: {} -> {}", url, dest.display());

    // Create parent directories if they don't exist
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    // Send the HTTP GET request
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

    // Stream the response body in chunks, writing each to disk
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
/// Handles both files and directories within the archive. On Unix systems,
/// executable permissions from the archive metadata are preserved. Uses
/// `spawn_blocking` because the `zip` crate is synchronous.
///
/// # Arguments
/// * `archive_path` - Path to the ZIP file to extract
/// * `dest` - Directory to extract contents into (created if it doesn't exist)
pub async fn extract_zip(archive_path: &Path, dest: &Path) -> Result<(), String> {
    log::info!(
        "Extracting ZIP: {} -> {}",
        archive_path.display(),
        dest.display()
    );

    // Create destination directory if it doesn't exist
    std::fs::create_dir_all(dest)
        .map_err(|e| format!("Failed to create directory {}: {}", dest.display(), e))?;

    // Clone paths for the blocking closure (can't move borrows into spawn_blocking)
    let archive_path = archive_path.to_path_buf();
    let dest = dest.to_path_buf();

    // Run synchronous ZIP extraction in a blocking thread pool task
    tokio::task::spawn_blocking(move || {
        // Open the ZIP file and create an archive reader
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

            // Use enclosed_name() for security - prevents path traversal attacks
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

                // Preserve executable permissions on Unix platforms
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = entry.unix_mode() {
                        // Only set permissions if the archive specifies them
                        std::fs::set_permissions(
                            &outpath,
                            std::fs::Permissions::from_mode(mode),
                        )
                        .ok(); // Best-effort; don't fail the extraction
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
/// Decompresses the gzip layer, then unpacks the tar archive. File
/// permissions are automatically preserved on Unix. Uses `spawn_blocking`
/// because `flate2` and `tar` crates are synchronous.
///
/// # Arguments
/// * `archive_path` - Path to the .tar.gz file to extract
/// * `dest` - Directory to extract contents into (created if it doesn't exist)
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
        // Open the .tar.gz file and wrap it in a gzip decoder
        let file = std::fs::File::open(&archive_path)
            .map_err(|e| format!("Failed to open archive {}: {}", archive_path.display(), e))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        // Set options to preserve permissions and handle long paths
        archive.set_preserve_permissions(true);
        archive.set_overwrite(true);

        // Unpack all entries to the destination directory
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
/// This is the primary entry point for installing dependencies. It handles
/// the complete download-and-extract pipeline:
/// 1. Downloads the archive to a temporary file in the system temp directory
/// 2. Extracts the archive to the destination using the appropriate extractor
/// 3. Cleans up the temporary download file
///
/// # Arguments
/// * `url` - The URL to download the archive from
/// * `dest` - The directory to extract the archive contents into
/// * `format` - The expected archive format (ZIP or TAR.GZ)
pub async fn download_and_extract(
    url: &str,
    dest: &Path,
    format: ArchiveFormat,
) -> Result<(), String> {
    // Derive a temp file name from the URL's filename component
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
/// On Unix systems, marks the file as executable by the owner (chmod u+x).
/// On Windows this is a no-op since executability is determined by file extension.
///
/// # Arguments
/// * `path` - Path to the file to mark as executable
#[allow(unused_variables)]
pub fn set_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(path)
            .map_err(|e| format!("Failed to read metadata for {}: {}", path.display(), e))?;
        let mut perms = metadata.permissions();
        // Add execute permission for owner, group, and others (matching original + execute)
        let mode = perms.mode() | 0o111;
        perms.set_mode(mode);
        std::fs::set_permissions(path, perms)
            .map_err(|e| format!("Failed to set permissions on {}: {}", path.display(), e))?;
        log::debug!("Set executable permission on {}", path.display());
    }
    Ok(())
}
