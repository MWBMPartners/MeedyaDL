#!/usr/bin/env bash
# Copyright (c) 2024-2026 MeedyaDL
# Licensed under the MIT License. See LICENSE file in the project root.
#
# Download Bundled Dependencies for MeedyaDL
# =============================================
#
# Downloads all external runtime dependencies for a specific platform/arch
# and stages them for Tauri resource bundling. Called by CI workflows
# (release.yml, pre-release.yml) before `cargo tauri build`.
#
# Usage:
#   bash scripts/download-bundled-deps.sh --os <os> --arch <arch> --output <dir>
#
# Arguments:
#   --os      Target OS: macos, windows, linux
#   --arch    Target arch: x86_64, aarch64, armv7
#   --output  Output directory (default: src-tauri/bundled-deps)
#
# Output structure:
#   <output>/
#   ├── python/          # Portable Python + GAMDL (pip-installed)
#   ├── tools/
#   │   ├── ffmpeg/      # FFmpeg binary
#   │   ├── mp4decrypt/  # mp4decrypt binary
#   │   ├── nm3u8dlre/   # N_m3u8DL-RE binary
#   │   └── mp4box/      # MP4Box binary (+ lib/ on macOS)
#   └── manifest.json    # Records which deps were successfully bundled
#
# The script is designed to be fault-tolerant: if a specific tool download
# fails, it logs the error and continues. The manifest.json records which
# tools were successfully bundled so the extraction service knows what to
# expect at runtime.
#
# Environment:
#   GITHUB_TOKEN  (optional) Used for GitHub API requests to avoid rate limits

set -euo pipefail

# ============================================================
# Constants
# ============================================================

# Python version and release tag (must match src-tauri/src/services/python_manager.rs)
PYTHON_VERSION="3.12.8"
PYTHON_RELEASE_TAG="20250106"
PYTHON_BASE_URL="https://github.com/indygreg/python-build-standalone/releases/download"

# Bento4 SDK version (must match dependency_manager.rs)
BENTO4_VERSION="1-6-0-641"

# ============================================================
# Argument Parsing
# ============================================================

TARGET_OS=""
TARGET_ARCH=""
OUTPUT_DIR="src-tauri/bundled-deps"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --os)     TARGET_OS="$2"; shift 2 ;;
        --arch)   TARGET_ARCH="$2"; shift 2 ;;
        --output) OUTPUT_DIR="$2"; shift 2 ;;
        *)        echo "Unknown argument: $1"; exit 1 ;;
    esac
done

if [[ -z "$TARGET_OS" || -z "$TARGET_ARCH" ]]; then
    echo "Usage: $0 --os <macos|windows|linux> --arch <x86_64|aarch64|armv7>"
    exit 1
fi

echo "=== MeedyaDL Bundled Dependencies Download ==="
echo "  Target: ${TARGET_OS}/${TARGET_ARCH}"
echo "  Output: ${OUTPUT_DIR}"
echo ""

# ============================================================
# Setup
# ============================================================

# Create output directory structure
mkdir -p "${OUTPUT_DIR}/python"
mkdir -p "${OUTPUT_DIR}/tools/ffmpeg"
mkdir -p "${OUTPUT_DIR}/tools/mp4decrypt"
mkdir -p "${OUTPUT_DIR}/tools/nm3u8dlre"
mkdir -p "${OUTPUT_DIR}/tools/mp4box"

# Temporary download directory
TMPDIR_DL="$(mktemp -d)"
trap 'rm -rf "${TMPDIR_DL}"' EXIT

# Track success/failure per dependency (simple variables for bash 3.2 compat)
MANIFEST_python="false"
MANIFEST_ffmpeg="false"
MANIFEST_mp4decrypt="false"
MANIFEST_nm3u8dlre="false"
MANIFEST_mp4box="false"

# GitHub API auth header (if token is available)
GH_AUTH_HEADER=""
if [[ -n "${GITHUB_TOKEN:-}" ]]; then
    GH_AUTH_HEADER="-H \"Authorization: token ${GITHUB_TOKEN}\""
fi

# ============================================================
# Helper Functions
# ============================================================

log_info() {
    echo "[INFO] $*"
}

log_error() {
    echo "[ERROR] $*" >&2
}

log_success() {
    echo "[OK]   $*"
}

# Download a file with progress
download_file() {
    local url="$1"
    local output="$2"
    log_info "Downloading: ${url}"
    if [[ -n "${GITHUB_TOKEN:-}" ]]; then
        curl -fSL --retry 3 --retry-delay 5 \
            -H "Authorization: token ${GITHUB_TOKEN}" \
            -H "User-Agent: MeedyaDL" \
            -o "${output}" "${url}"
    else
        curl -fSL --retry 3 --retry-delay 5 \
            -H "User-Agent: MeedyaDL" \
            -o "${output}" "${url}"
    fi
}

# Query GitHub Releases API for an asset URL
# Usage: resolve_github_asset <repo> <tag> <asset_substring>
# Outputs the browser_download_url to stdout
resolve_github_asset() {
    local repo="$1"
    local tag="$2"
    local asset_match="$3"

    local api_url
    if [[ "$tag" == "latest" ]]; then
        api_url="https://api.github.com/repos/${repo}/releases/latest"
    else
        api_url="https://api.github.com/repos/${repo}/releases/tags/${tag}"
    fi

    local response
    if [[ -n "${GITHUB_TOKEN:-}" ]]; then
        response=$(curl -fsSL \
            -H "Authorization: token ${GITHUB_TOKEN}" \
            -H "User-Agent: MeedyaDL" \
            "${api_url}")
    else
        response=$(curl -fsSL \
            -H "User-Agent: MeedyaDL" \
            "${api_url}")
    fi

    # Find the matching asset URL using grep/sed (portable, no jq dependency)
    echo "${response}" | grep -o '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]*'"${asset_match}"'[^"]*"' \
        | head -1 \
        | sed 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/'
}

# ============================================================
# 1. Python + GAMDL
# ============================================================

download_python() {
    log_info "=== Downloading Python ${PYTHON_VERSION} ==="

    # Map target to python-build-standalone triple
    local triple
    case "${TARGET_OS}-${TARGET_ARCH}" in
        macos-aarch64)    triple="aarch64-apple-darwin" ;;
        macos-x86_64)     triple="x86_64-apple-darwin" ;;
        linux-x86_64)     triple="x86_64-unknown-linux-gnu" ;;
        linux-aarch64)    triple="aarch64-unknown-linux-gnu" ;;
        windows-x86_64)   triple="x86_64-pc-windows-msvc" ;;
        windows-aarch64)  triple="aarch64-pc-windows-msvc" ;;
        *)
            log_error "No Python build available for ${TARGET_OS}/${TARGET_ARCH}"
            MANIFEST_python="false"
            return 1
            ;;
    esac

    local archive_name="cpython-${PYTHON_VERSION}+${PYTHON_RELEASE_TAG}-${triple}-install_only.tar.gz"
    local url="${PYTHON_BASE_URL}/${PYTHON_RELEASE_TAG}/${archive_name}"
    local archive_path="${TMPDIR_DL}/${archive_name}"

    download_file "${url}" "${archive_path}" || { MANIFEST_python="false"; return 1; }

    # Extract Python to output directory
    log_info "Extracting Python..."
    tar xzf "${archive_path}" -C "${OUTPUT_DIR}/python" --strip-components=1

    # Install GAMDL into the Python environment
    install_gamdl || { MANIFEST_python="false"; return 1; }

    log_success "Python ${PYTHON_VERSION} + GAMDL bundled"
    MANIFEST_python="true"
}

install_gamdl() {
    log_info "Installing GAMDL into bundled Python..."

    local python_bin
    if [[ "${TARGET_OS}" == "windows" ]]; then
        python_bin="${OUTPUT_DIR}/python/python.exe"
    else
        python_bin="${OUTPUT_DIR}/python/bin/python3"
    fi

    # For native builds (host arch == target arch), run pip directly
    # For cross-compiled builds, use --target to install pure-Python packages
    local host_arch
    host_arch=$(uname -m)
    # Normalize host arch naming
    case "${host_arch}" in
        arm64)  host_arch="aarch64" ;;
        x86_64) host_arch="x86_64" ;;
    esac

    if [[ "${host_arch}" == "${TARGET_ARCH}" && -x "${python_bin}" ]]; then
        # Native: run the downloaded Python directly
        log_info "Installing GAMDL (native pip install)..."
        "${python_bin}" -m pip install --upgrade pip --quiet 2>/dev/null || true
        "${python_bin}" -m pip install gamdl --quiet
    else
        # Cross-compile: use host Python with --target pointing to target's site-packages
        log_info "Installing GAMDL (cross-arch via host Python)..."

        # Determine the target site-packages directory
        local site_packages
        if [[ "${TARGET_OS}" == "windows" ]]; then
            site_packages="${OUTPUT_DIR}/python/Lib/site-packages"
        else
            # Find the python3.XX directory
            local py_lib_dir
            py_lib_dir=$(find "${OUTPUT_DIR}/python/lib" -maxdepth 1 -name "python3.*" -type d | head -1)
            if [[ -z "${py_lib_dir}" ]]; then
                log_error "Could not find Python lib directory for cross-arch install"
                return 1
            fi
            site_packages="${py_lib_dir}/site-packages"
        fi

        mkdir -p "${site_packages}"
        python3 -m pip install gamdl --target="${site_packages}" --quiet 2>/dev/null || \
        python -m pip install gamdl --target="${site_packages}" --quiet
    fi
}

# ============================================================
# 2. FFmpeg
# ============================================================

download_ffmpeg() {
    log_info "=== Downloading FFmpeg ==="

    local url=""
    local format=""

    case "${TARGET_OS}-${TARGET_ARCH}" in
        linux-x86_64)
            url="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz"
            format="tar.xz"
            ;;
        windows-x86_64|windows-aarch64)
            # x64 binary works on ARM64 via Windows emulation
            url="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
            format="zip"
            ;;
        macos-x86_64|macos-aarch64)
            url="https://evermeet.cx/ffmpeg/getrelease/zip"
            format="zip"
            ;;
        *)
            log_error "No FFmpeg build for ${TARGET_OS}/${TARGET_ARCH} — will skip"
            MANIFEST_ffmpeg="false"
            return 0
            ;;
    esac

    local archive_path="${TMPDIR_DL}/ffmpeg.${format}"
    download_file "${url}" "${archive_path}" || { MANIFEST_ffmpeg="false"; return 0; }

    log_info "Extracting FFmpeg..."
    local extract_dir="${TMPDIR_DL}/ffmpeg_extract"
    mkdir -p "${extract_dir}"

    case "${format}" in
        tar.xz)
            tar xJf "${archive_path}" -C "${extract_dir}"
            ;;
        zip)
            unzip -qo "${archive_path}" -d "${extract_dir}"
            ;;
    esac

    # Find the ffmpeg binary recursively
    local binary_name="ffmpeg"
    [[ "${TARGET_OS}" == "windows" ]] && binary_name="ffmpeg.exe"

    local found_binary
    found_binary=$(find "${extract_dir}" -name "${binary_name}" -type f | head -1)

    if [[ -n "${found_binary}" ]]; then
        cp "${found_binary}" "${OUTPUT_DIR}/tools/ffmpeg/${binary_name}"
        chmod +x "${OUTPUT_DIR}/tools/ffmpeg/${binary_name}" 2>/dev/null || true
        log_success "FFmpeg bundled"
        MANIFEST_ffmpeg="true"
    else
        log_error "FFmpeg binary not found in extracted archive"
        MANIFEST_ffmpeg="false"
    fi
}

# ============================================================
# 3. mp4decrypt (Bento4)
# ============================================================

download_mp4decrypt() {
    log_info "=== Downloading mp4decrypt (Bento4) ==="

    local platform_suffix=""
    case "${TARGET_OS}" in
        macos)   platform_suffix="universal-apple-macosx" ;;
        linux)   platform_suffix="x86_64-unknown-linux" ;;
        windows) platform_suffix="x86_64-microsoft-win32" ;;
    esac

    if [[ -z "${platform_suffix}" ]]; then
        log_error "No mp4decrypt build for ${TARGET_OS}/${TARGET_ARCH}"
        MANIFEST_mp4decrypt="false"
        return 0
    fi

    local url="https://www.bok.net/Bento4/binaries/Bento4-SDK-${BENTO4_VERSION}.${platform_suffix}.zip"
    local archive_path="${TMPDIR_DL}/bento4.zip"

    download_file "${url}" "${archive_path}" || { MANIFEST_mp4decrypt="false"; return 0; }

    log_info "Extracting mp4decrypt..."
    local extract_dir="${TMPDIR_DL}/bento4_extract"
    mkdir -p "${extract_dir}"
    unzip -qo "${archive_path}" -d "${extract_dir}"

    local binary_name="mp4decrypt"
    [[ "${TARGET_OS}" == "windows" ]] && binary_name="mp4decrypt.exe"

    local found_binary
    found_binary=$(find "${extract_dir}" -name "${binary_name}" -type f | head -1)

    if [[ -n "${found_binary}" ]]; then
        cp "${found_binary}" "${OUTPUT_DIR}/tools/mp4decrypt/${binary_name}"
        chmod +x "${OUTPUT_DIR}/tools/mp4decrypt/${binary_name}" 2>/dev/null || true
        log_success "mp4decrypt bundled"
        MANIFEST_mp4decrypt="true"
    else
        log_error "mp4decrypt binary not found in Bento4 SDK"
        MANIFEST_mp4decrypt="false"
    fi
}

# ============================================================
# 4. N_m3u8DL-RE
# ============================================================

download_nm3u8dlre() {
    log_info "=== Downloading N_m3u8DL-RE ==="

    # Map to .NET Runtime Identifier (RID)
    local rid=""
    case "${TARGET_OS}-${TARGET_ARCH}" in
        macos-aarch64)    rid="osx-arm64" ;;
        macos-x86_64)     rid="osx-x64" ;;
        linux-x86_64)     rid="linux-x64" ;;
        linux-aarch64)    rid="linux-arm64" ;;
        windows-x86_64)   rid="win-x64" ;;
        windows-aarch64)  rid="win-arm64" ;;
        *)
            log_error "No N_m3u8DL-RE build for ${TARGET_OS}/${TARGET_ARCH}"
            MANIFEST_nm3u8dlre="false"
            return 0
            ;;
    esac

    # Resolve the latest release asset URL via GitHub API
    local url
    url=$(resolve_github_asset "nilaoda/N_m3u8DL-RE" "latest" "${rid}")

    if [[ -z "${url}" ]]; then
        log_error "Could not resolve N_m3u8DL-RE asset for RID: ${rid}"
        MANIFEST_nm3u8dlre="false"
        return 0
    fi

    # Determine archive format from URL
    local format="tar.gz"
    [[ "${url}" == *.zip ]] && format="zip"

    local archive_path="${TMPDIR_DL}/nm3u8dlre.${format}"
    download_file "${url}" "${archive_path}" || { MANIFEST_nm3u8dlre="false"; return 0; }

    log_info "Extracting N_m3u8DL-RE..."
    local extract_dir="${TMPDIR_DL}/nm3u8dlre_extract"
    mkdir -p "${extract_dir}"

    case "${format}" in
        tar.gz)  tar xzf "${archive_path}" -C "${extract_dir}" ;;
        zip)     unzip -qo "${archive_path}" -d "${extract_dir}" ;;
    esac

    # Find binary (case-insensitive search)
    local binary_name="N_m3u8DL-RE"
    [[ "${TARGET_OS}" == "windows" ]] && binary_name="N_m3u8DL-RE.exe"

    local found_binary
    found_binary=$(find "${extract_dir}" -iname "${binary_name}" -type f | head -1)

    if [[ -n "${found_binary}" ]]; then
        cp "${found_binary}" "${OUTPUT_DIR}/tools/nm3u8dlre/${binary_name}"
        chmod +x "${OUTPUT_DIR}/tools/nm3u8dlre/${binary_name}" 2>/dev/null || true
        log_success "N_m3u8DL-RE bundled"
        MANIFEST_nm3u8dlre="true"
    else
        log_error "N_m3u8DL-RE binary not found in extracted archive"
        MANIFEST_nm3u8dlre="false"
    fi
}

# ============================================================
# 5. MP4Box (GPAC)
# ============================================================

download_mp4box() {
    log_info "=== Downloading MP4Box (GPAC) ==="

    local binary_name="MP4Box"
    [[ "${TARGET_OS}" == "windows" ]] && binary_name="MP4Box.exe"

    case "${TARGET_OS}" in
        macos)
            download_mp4box_macos
            ;;
        windows)
            download_mp4box_windows
            ;;
        linux)
            download_mp4box_linux
            ;;
        *)
            log_error "No MP4Box build for ${TARGET_OS}"
            MANIFEST_mp4box="false"
            return 0
            ;;
    esac
}

download_mp4box_macos() {
    # Download GPAC .pkg and extract MP4Box
    local url="https://download.tsi.telecom-paristech.fr/gpac/new_builds/gpac_latest_head_macos.pkg"
    local pkg_path="${TMPDIR_DL}/gpac.pkg"

    download_file "${url}" "${pkg_path}" || { MANIFEST_mp4box="false"; return 0; }

    log_info "Extracting MP4Box from .pkg..."
    local expand_dir="${TMPDIR_DL}/gpac_pkg"
    # pkgutil --expand requires the destination to NOT exist
    rm -rf "${expand_dir}"

    # Expand the .pkg archive (macOS only — CI uses macOS runners for macOS targets)
    pkgutil --expand "${pkg_path}" "${expand_dir}" 2>/dev/null || {
        log_error "Failed to expand GPAC .pkg (pkgutil not available or .pkg format unsupported)"
        MANIFEST_mp4box="false"
        return 0
    }

    # Find and extract the Payload
    local payload
    payload=$(find "${expand_dir}" -name "Payload" -type f | head -1)

    if [[ -z "${payload}" ]]; then
        log_error "No Payload found in GPAC .pkg"
        MANIFEST_mp4box="false"
        return 0
    fi

    local payload_dir="${TMPDIR_DL}/gpac_payload"
    mkdir -p "${payload_dir}"
    cd "${payload_dir}"
    gunzip -c "${payload}" | cpio -id 2>/dev/null
    cd - > /dev/null

    # Find MP4Box binary
    local found_binary
    found_binary=$(find "${payload_dir}" -name "MP4Box" -type f | head -1)

    if [[ -n "${found_binary}" ]]; then
        cp "${found_binary}" "${OUTPUT_DIR}/tools/mp4box/MP4Box"
        chmod +x "${OUTPUT_DIR}/tools/mp4box/MP4Box"

        # Also copy the lib/ directory (for @executable_path/lib/libgpac.dylib)
        local found_dir
        found_dir=$(dirname "${found_binary}")
        if [[ -d "${found_dir}/lib" ]]; then
            cp -R "${found_dir}/lib" "${OUTPUT_DIR}/tools/mp4box/lib"
        elif [[ -d "${found_dir}/../lib" ]]; then
            cp -R "${found_dir}/../lib" "${OUTPUT_DIR}/tools/mp4box/lib"
        fi

        log_success "MP4Box bundled (macOS .pkg)"
        MANIFEST_mp4box="true"
    else
        log_error "MP4Box binary not found in GPAC .pkg"
        MANIFEST_mp4box="false"
    fi
}

download_mp4box_windows() {
    local url="https://download.tsi.telecom-paristech.fr/gpac/new_builds/gpac_latest_head_win64.exe"
    local installer_path="${TMPDIR_DL}/gpac_installer.exe"

    download_file "${url}" "${installer_path}" || { MANIFEST_mp4box="false"; return 0; }

    log_info "Extracting MP4Box from NSIS installer..."
    local install_dir="${TMPDIR_DL}/gpac_win"
    mkdir -p "${install_dir}"

    # Run NSIS installer silently with custom install path
    # Note: This only works on Windows runners
    "${installer_path}" /S "/D=${install_dir}" 2>/dev/null || {
        # On non-Windows runners (cross-compilation), try 7z extraction instead
        if command -v 7z &>/dev/null; then
            7z x -y -o"${install_dir}" "${installer_path}" >/dev/null 2>&1 || {
                log_error "Failed to extract GPAC installer"
                MANIFEST_mp4box="false"
                return 0
            }
        else
            log_error "Cannot extract NSIS installer (not on Windows and 7z not available)"
            MANIFEST_mp4box="false"
            return 0
        fi
    }

    local found_binary
    found_binary=$(find "${install_dir}" -iname "MP4Box.exe" -type f | head -1)

    if [[ -n "${found_binary}" ]]; then
        cp "${found_binary}" "${OUTPUT_DIR}/tools/mp4box/MP4Box.exe"
        log_success "MP4Box bundled (Windows)"
        MANIFEST_mp4box="true"
    else
        log_error "MP4Box.exe not found in GPAC installer"
        MANIFEST_mp4box="false"
    fi
}

download_mp4box_linux() {
    local url="https://download.tsi.telecom-paristech.fr/gpac/new_builds/gpac_latest_head_linux64.deb"
    local deb_path="${TMPDIR_DL}/gpac.deb"

    # Linux ARM builds don't have x64 .deb available
    if [[ "${TARGET_ARCH}" != "x86_64" ]]; then
        log_error "No MP4Box .deb for ${TARGET_ARCH} — skipping (will be installed at runtime)"
        MANIFEST_mp4box="false"
        return 0
    fi

    download_file "${url}" "${deb_path}" || { MANIFEST_mp4box="false"; return 0; }

    log_info "Extracting MP4Box from .deb..."
    local extract_dir="${TMPDIR_DL}/gpac_deb"
    mkdir -p "${extract_dir}"
    cd "${extract_dir}"

    # Unpack the .deb (ar archive) and extract the data payload
    ar x "${deb_path}" 2>/dev/null
    mkdir -p data
    tar xf data.tar.* -C data/ 2>/dev/null

    cd - > /dev/null

    local found_binary
    found_binary=$(find "${extract_dir}/data" -name "MP4Box" -type f | head -1)
    # Also try lowercase
    [[ -z "${found_binary}" ]] && found_binary=$(find "${extract_dir}/data" -name "mp4box" -type f | head -1)

    if [[ -n "${found_binary}" ]]; then
        cp "${found_binary}" "${OUTPUT_DIR}/tools/mp4box/MP4Box"
        chmod +x "${OUTPUT_DIR}/tools/mp4box/MP4Box"
        log_success "MP4Box bundled (Linux .deb)"
        MANIFEST_mp4box="true"
    else
        log_error "MP4Box binary not found in GPAC .deb"
        MANIFEST_mp4box="false"
    fi
}

# ============================================================
# Write Manifest
# ============================================================

write_manifest() {
    log_info "=== Writing manifest.json ==="

    local manifest_path="${OUTPUT_DIR}/manifest.json"

    # Build JSON manually (no jq dependency)
    cat > "${manifest_path}" << EOF
{
  "bundled_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "target_os": "${TARGET_OS}",
  "target_arch": "${TARGET_ARCH}",
  "python_version": "${PYTHON_VERSION}",
  "dependencies": {
    "python": ${MANIFEST_python},
    "ffmpeg": ${MANIFEST_ffmpeg},
    "mp4decrypt": ${MANIFEST_mp4decrypt},
    "nm3u8dlre": ${MANIFEST_nm3u8dlre},
    "mp4box": ${MANIFEST_mp4box}
  }
}
EOF

    log_info "Manifest written to ${manifest_path}"
}

# ============================================================
# Main
# ============================================================

main() {
    # Download each dependency (failures are logged, not fatal)
    download_python || true
    download_ffmpeg || true
    download_mp4decrypt || true
    download_nm3u8dlre || true
    download_mp4box || true

    # Write the manifest
    write_manifest

    # Summary
    echo ""
    echo "=== Bundle Summary ==="
    echo "  Python:      ${MANIFEST_python}"
    echo "  FFmpeg:       ${MANIFEST_ffmpeg}"
    echo "  mp4decrypt:   ${MANIFEST_mp4decrypt}"
    echo "  N_m3u8DL-RE:  ${MANIFEST_nm3u8dlre}"
    echo "  MP4Box:       ${MANIFEST_mp4box}"
    echo ""

    # Create tar.gz archive for Tauri resource bundling
    # (Tauri's resource glob flattens directory structure, so we bundle as a
    # single archive and extract at runtime in bundled_deps_service.rs)
    log_info "=== Creating bundled-deps.tar.gz ==="
    local archive_path="${OUTPUT_DIR}.tar.gz"
    tar czf "${archive_path}" -C "$(dirname "${OUTPUT_DIR}")" "$(basename "${OUTPUT_DIR}")"
    log_success "Archive created: ${archive_path}"

    # Calculate total size
    local total_size
    total_size=$(du -sh "${OUTPUT_DIR}" 2>/dev/null | cut -f1)
    local archive_size
    archive_size=$(du -sh "${archive_path}" 2>/dev/null | cut -f1)
    echo "  Total size:   ${total_size} (uncompressed)"
    echo "  Archive size: ${archive_size} (compressed)"
    echo ""
}

main
