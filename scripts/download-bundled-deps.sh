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
#   ├── python/          # Portable Python + pip packages (gamdl, votify, gytmdl, yt-dlp)
#   ├── tools/
#   │   ├── ffmpeg/      # FFmpeg binary
#   │   ├── mp4decrypt/  # mp4decrypt binary
#   │   ├── nm3u8dlre/   # N_m3u8DL-RE binary
#   │   ├── mp4box/      # MP4Box binary (+ lib/ on macOS)
#   │   ├── aria2c/      # aria2c binary (Windows only; optional)
#   │   ├── fpcalc/      # fpcalc binary (optional AcousticID fallback)
#   │   ├── amdecrypt/   # amdecrypt binary (optional; from mirror)
#   │   └── get_iplayer/ # get_iplayer Perl script (BBC iPlayer)
#   ├── perl/            # Portable Perl runtime (for get_iplayer)
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
mkdir -p "${OUTPUT_DIR}/perl"
mkdir -p "${OUTPUT_DIR}/tools/ffmpeg"
mkdir -p "${OUTPUT_DIR}/tools/mp4decrypt"
mkdir -p "${OUTPUT_DIR}/tools/nm3u8dlre"
mkdir -p "${OUTPUT_DIR}/tools/mp4box"
mkdir -p "${OUTPUT_DIR}/tools/aria2c"
mkdir -p "${OUTPUT_DIR}/tools/fpcalc"
mkdir -p "${OUTPUT_DIR}/tools/amdecrypt"
mkdir -p "${OUTPUT_DIR}/tools/get_iplayer"

# Temporary download directory
TMPDIR_DL="$(mktemp -d)"
trap 'rm -rf "${TMPDIR_DL}"' EXIT

# Track success/failure per dependency (simple variables for bash 3.2 compat)
MANIFEST_python="false"
MANIFEST_gamdl="false"
MANIFEST_votify="false"
MANIFEST_gytmdl="false"
MANIFEST_ytdlp="false"
MANIFEST_ffmpeg="false"
MANIFEST_mp4decrypt="false"
MANIFEST_nm3u8dlre="false"
MANIFEST_mp4box="false"
MANIFEST_aria2c="false"
MANIFEST_fpcalc="false"
MANIFEST_amdecrypt="false"
MANIFEST_perl="false"
MANIFEST_get_iplayer="false"

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

    # Install Python packages into the environment
    install_python_packages || { MANIFEST_python="false"; return 1; }

    log_success "Python ${PYTHON_VERSION} + packages bundled"
    MANIFEST_python="true"
}

install_python_packages() {
    log_info "Installing Python packages into bundled Python..."

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

    # Define all Python packages to install
    # Each package is installed individually so we can track success/failure per package
    local packages="gamdl votify gytmdl yt-dlp"

    if [[ "${host_arch}" == "${TARGET_ARCH}" && -x "${python_bin}" ]]; then
        # Native: run the downloaded Python directly
        log_info "Installing packages (native pip install)..."
        "${python_bin}" -m pip install --upgrade pip --quiet 2>/dev/null || true

        for pkg in ${packages}; do
            log_info "Installing ${pkg}..."
            if "${python_bin}" -m pip install "${pkg}" --quiet; then
                log_success "${pkg} installed"
                set_package_manifest "${pkg}" "true"
            else
                log_error "Failed to install ${pkg}"
                set_package_manifest "${pkg}" "false"
            fi
        done
    else
        # Cross-compile: use host Python with --target pointing to target's site-packages
        log_info "Installing packages (cross-arch via host Python)..."

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

        for pkg in ${packages}; do
            log_info "Installing ${pkg} (cross-arch)..."
            if python3 -m pip install "${pkg}" --target="${site_packages}" --quiet 2>/dev/null || \
               python -m pip install "${pkg}" --target="${site_packages}" --quiet 2>/dev/null; then
                log_success "${pkg} installed"
                set_package_manifest "${pkg}" "true"
            else
                log_error "Failed to install ${pkg}"
                set_package_manifest "${pkg}" "false"
            fi
        done
    fi
}

# Helper to set manifest variable for a Python package by name
set_package_manifest() {
    local pkg="$1"
    local value="$2"
    case "${pkg}" in
        gamdl)   MANIFEST_gamdl="${value}" ;;
        votify)  MANIFEST_votify="${value}" ;;
        gytmdl)  MANIFEST_gytmdl="${value}" ;;
        yt-dlp)  MANIFEST_ytdlp="${value}" ;;
    esac
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

    # Always use 7z to extract the NSIS installer (available on all GitHub Windows runners).
    # The NSIS /S (silent) flag hangs indefinitely in CI because GPAC's installer ignores it
    # and opens a GUI dialog waiting for user input. 7z can extract NSIS .exe archives directly.
    if command -v 7z &>/dev/null; then
        7z x -y -o"${install_dir}" "${installer_path}" >/dev/null 2>&1 || {
            log_error "Failed to extract GPAC installer via 7z"
            MANIFEST_mp4box="false"
            return 0
        }
    else
        log_error "7z not available — cannot extract GPAC NSIS installer in CI"
        MANIFEST_mp4box="false"
        return 0
    fi

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
# 6. aria2c (optional download accelerator)
# Source: aria2/aria2 GitHub releases (Windows only)
# macOS/Linux: not available as pre-built upstream binaries
# ============================================================

download_aria2c() {
    log_info "=== Downloading aria2c ==="

    # Only Windows has pre-built binaries from upstream
    if [[ "${TARGET_OS}" != "windows" ]]; then
        log_info "aria2c: No pre-built binary for ${TARGET_OS} — skipping (install via Homebrew/apt)"
        MANIFEST_aria2c="false"
        return 0
    fi

    # Resolve the latest release asset from GitHub
    local url
    url=$(resolve_github_asset "aria2/aria2" "latest" "win-64bit")

    if [[ -z "${url}" ]]; then
        log_error "Could not resolve aria2c asset for Windows x64"
        MANIFEST_aria2c="false"
        return 0
    fi

    local archive_path="${TMPDIR_DL}/aria2c.zip"
    download_file "${url}" "${archive_path}" || { MANIFEST_aria2c="false"; return 0; }

    log_info "Extracting aria2c..."
    local extract_dir="${TMPDIR_DL}/aria2c_extract"
    mkdir -p "${extract_dir}"
    unzip -qo "${archive_path}" -d "${extract_dir}"

    local found_binary
    found_binary=$(find "${extract_dir}" -name "aria2c.exe" -type f | head -1)

    if [[ -n "${found_binary}" ]]; then
        cp "${found_binary}" "${OUTPUT_DIR}/tools/aria2c/aria2c.exe"
        log_success "aria2c bundled (Windows)"
        MANIFEST_aria2c="true"
    else
        log_error "aria2c.exe not found in archive"
        MANIFEST_aria2c="false"
    fi
}

# ============================================================
# 7. fpcalc (Chromaprint — optional AcousticID fingerprinting)
# Source: acoustid/chromaprint GitHub releases
# ============================================================

download_fpcalc() {
    log_info "=== Downloading fpcalc (Chromaprint) ==="

    # Map to chromaprint asset naming
    local asset_match=""
    case "${TARGET_OS}-${TARGET_ARCH}" in
        macos-aarch64|macos-x86_64) asset_match="macos-universal" ;;
        linux-x86_64)               asset_match="linux-x86_64" ;;
        linux-aarch64)              asset_match="linux-arm64" ;;
        windows-x86_64)             asset_match="windows-x86_64" ;;
        *)
            log_info "fpcalc: No pre-built binary for ${TARGET_OS}/${TARGET_ARCH} — skipping"
            MANIFEST_fpcalc="false"
            return 0
            ;;
    esac

    local url
    url=$(resolve_github_asset "acoustid/chromaprint" "latest" "${asset_match}")

    if [[ -z "${url}" ]]; then
        log_error "Could not resolve fpcalc asset matching '${asset_match}'"
        MANIFEST_fpcalc="false"
        return 0
    fi

    # Determine archive format from URL
    local format="tar.gz"
    [[ "${url}" == *.zip ]] && format="zip"

    local archive_path="${TMPDIR_DL}/fpcalc.${format}"
    download_file "${url}" "${archive_path}" || { MANIFEST_fpcalc="false"; return 0; }

    log_info "Extracting fpcalc..."
    local extract_dir="${TMPDIR_DL}/fpcalc_extract"
    mkdir -p "${extract_dir}"

    case "${format}" in
        tar.gz)  tar xzf "${archive_path}" -C "${extract_dir}" ;;
        zip)     unzip -qo "${archive_path}" -d "${extract_dir}" ;;
    esac

    local binary_name="fpcalc"
    [[ "${TARGET_OS}" == "windows" ]] && binary_name="fpcalc.exe"

    local found_binary
    found_binary=$(find "${extract_dir}" -name "${binary_name}" -type f | head -1)

    if [[ -n "${found_binary}" ]]; then
        cp "${found_binary}" "${OUTPUT_DIR}/tools/fpcalc/${binary_name}"
        chmod +x "${OUTPUT_DIR}/tools/fpcalc/${binary_name}" 2>/dev/null || true
        log_success "fpcalc bundled"
        MANIFEST_fpcalc="true"
    else
        log_error "fpcalc binary not found in extracted archive"
        MANIFEST_fpcalc="false"
    fi
}

# ============================================================
# 8. amdecrypt (optional — from MeedyaDL-Tools mirror)
# No public upstream; assets are manually uploaded to mirror
# ============================================================

download_amdecrypt() {
    log_info "=== Downloading amdecrypt (from mirror) ==="

    local os_name=""
    local arch_name="${TARGET_ARCH}"
    case "${TARGET_OS}" in
        macos)   os_name="macos" ;;
        linux)   os_name="linux" ;;
        windows) os_name="windows" ;;
    esac

    local ext="tar.gz"
    [[ "${TARGET_OS}" == "windows" ]] && ext="zip"

    local asset_match="amdecrypt-${os_name}-${arch_name}"
    local url
    url=$(resolve_github_asset "MeedyaDL/MeedyaDL-Tools" "latest" "${asset_match}")

    if [[ -z "${url}" ]]; then
        log_info "amdecrypt: Not available in mirror for ${os_name}/${arch_name} — skipping (optional)"
        MANIFEST_amdecrypt="false"
        return 0
    fi

    local archive_path="${TMPDIR_DL}/amdecrypt.${ext}"
    download_file "${url}" "${archive_path}" || { MANIFEST_amdecrypt="false"; return 0; }

    log_info "Extracting amdecrypt..."
    local extract_dir="${TMPDIR_DL}/amdecrypt_extract"
    mkdir -p "${extract_dir}"

    case "${ext}" in
        tar.gz)  tar xzf "${archive_path}" -C "${extract_dir}" ;;
        zip)     unzip -qo "${archive_path}" -d "${extract_dir}" ;;
    esac

    local binary_name="amdecrypt"
    [[ "${TARGET_OS}" == "windows" ]] && binary_name="amdecrypt.exe"

    local found_binary
    found_binary=$(find "${extract_dir}" -name "${binary_name}" -type f | head -1)

    if [[ -n "${found_binary}" ]]; then
        cp "${found_binary}" "${OUTPUT_DIR}/tools/amdecrypt/${binary_name}"
        chmod +x "${OUTPUT_DIR}/tools/amdecrypt/${binary_name}" 2>/dev/null || true
        log_success "amdecrypt bundled"
        MANIFEST_amdecrypt="true"
    else
        log_error "amdecrypt binary not found in archive"
        MANIFEST_amdecrypt="false"
    fi
}

# ============================================================
# 9. Perl + get_iplayer (BBC iPlayer support)
# Source: skaji/relocatable-perl (macOS/Linux)
#         get-iplayer/get_iplayer (Perl script from GitHub)
# Note: Windows uses Strawberry Perl Portable (not yet implemented)
# ============================================================

download_perl() {
    log_info "=== Downloading Perl (relocatable) ==="

    # Map to skaji/relocatable-perl asset naming
    local asset_match=""
    case "${TARGET_OS}-${TARGET_ARCH}" in
        macos-aarch64)  asset_match="perl-darwin-arm64.tar.gz" ;;
        macos-x86_64)   asset_match="perl-darwin-amd64.tar.gz" ;;
        linux-x86_64)   asset_match="perl-linux-amd64.tar.gz" ;;
        linux-aarch64)  asset_match="perl-linux-arm64.tar.gz" ;;
        windows-*)
            log_info "Perl: Windows bundling not yet implemented — skipping"
            MANIFEST_perl="false"
            return 0
            ;;
        *)
            log_error "No relocatable Perl for ${TARGET_OS}/${TARGET_ARCH}"
            MANIFEST_perl="false"
            return 0
            ;;
    esac

    local url
    url=$(resolve_github_asset "skaji/relocatable-perl" "latest" "${asset_match}")

    if [[ -z "${url}" ]]; then
        log_error "Could not resolve Perl asset matching '${asset_match}'"
        MANIFEST_perl="false"
        return 0
    fi

    local archive_path="${TMPDIR_DL}/perl.tar.gz"
    download_file "${url}" "${archive_path}" || { MANIFEST_perl="false"; return 0; }

    log_info "Extracting Perl..."
    tar xzf "${archive_path}" -C "${OUTPUT_DIR}/perl" --strip-components=1

    # Verify the perl binary exists
    local perl_bin="${OUTPUT_DIR}/perl/bin/perl"
    if [[ ! -x "${perl_bin}" ]]; then
        log_error "Perl binary not found after extraction"
        MANIFEST_perl="false"
        return 0
    fi

    log_success "Perl bundled"
    MANIFEST_perl="true"

    # Install get_iplayer and its dependencies into the bundled Perl
    install_get_iplayer || { MANIFEST_get_iplayer="false"; return 0; }
}

install_get_iplayer() {
    log_info "Installing get_iplayer into bundled Perl..."

    local perl_bin="${OUTPUT_DIR}/perl/bin/perl"

    # For native builds, run the bundled Perl directly
    local host_arch
    host_arch=$(uname -m)
    case "${host_arch}" in
        arm64)  host_arch="aarch64" ;;
        x86_64) host_arch="x86_64" ;;
    esac

    if [[ "${host_arch}" != "${TARGET_ARCH}" ]]; then
        log_info "Cross-arch: skipping get_iplayer CPAN module installation"
        log_info "CPAN modules will be installed at first launch"

        # Still download the get_iplayer script
        download_get_iplayer_script || return 1
        return 0
    fi

    # Install cpanm (CPAN module installer) into the bundled Perl
    log_info "Installing cpanm..."
    local cpanm_url="https://cpanmin.us"
    local cpanm_path="${TMPDIR_DL}/cpanm"
    download_file "${cpanm_url}" "${cpanm_path}" || {
        log_error "Failed to download cpanm"
        return 1
    }
    chmod +x "${cpanm_path}"

    # Install required CPAN modules for get_iplayer
    # LWP::UserAgent — HTTP client
    # XML::LibXML — XML parsing (requires libxml2-dev on CI)
    # Mojolicious — Web framework (used by get_iplayer for recording)
    # CGI — CGI support (used by get_iplayer web PVR)
    local modules="LWP::UserAgent XML::LibXML Mojolicious CGI"
    log_info "Installing CPAN modules: ${modules}"

    for module in ${modules}; do
        log_info "Installing ${module}..."
        "${perl_bin}" "${cpanm_path}" --notest "${module}" 2>/dev/null || {
            log_error "Failed to install CPAN module: ${module}"
            # Continue with other modules — some are optional
        }
    done

    # Download the get_iplayer script
    download_get_iplayer_script || return 1

    log_success "get_iplayer installed"
    MANIFEST_get_iplayer="true"
}

download_get_iplayer_script() {
    log_info "Downloading get_iplayer script..."

    # Download the main get_iplayer script from the latest release
    local url
    url=$(resolve_github_asset "get-iplayer/get_iplayer" "latest" "get_iplayer")

    if [[ -z "${url}" ]]; then
        # Fallback: download from the raw GitHub content (main branch)
        url="https://raw.githubusercontent.com/get-iplayer/get_iplayer/master/get_iplayer"
    fi

    local script_path="${OUTPUT_DIR}/tools/get_iplayer/get_iplayer"
    download_file "${url}" "${script_path}" || {
        log_error "Failed to download get_iplayer script"
        return 1
    }

    chmod +x "${script_path}"
    log_success "get_iplayer script downloaded"
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
    "gamdl": ${MANIFEST_gamdl},
    "votify": ${MANIFEST_votify},
    "gytmdl": ${MANIFEST_gytmdl},
    "ytdlp": ${MANIFEST_ytdlp},
    "ffmpeg": ${MANIFEST_ffmpeg},
    "mp4decrypt": ${MANIFEST_mp4decrypt},
    "nm3u8dlre": ${MANIFEST_nm3u8dlre},
    "mp4box": ${MANIFEST_mp4box},
    "aria2c": ${MANIFEST_aria2c},
    "fpcalc": ${MANIFEST_fpcalc},
    "amdecrypt": ${MANIFEST_amdecrypt},
    "perl": ${MANIFEST_perl},
    "get_iplayer": ${MANIFEST_get_iplayer}
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
    download_aria2c || true
    download_fpcalc || true
    download_amdecrypt || true
    download_perl || true

    # Write the manifest
    write_manifest

    # Summary
    echo ""
    echo "=== Bundle Summary ==="
    echo "  Python:       ${MANIFEST_python}"
    echo "  --- Python Packages ---"
    echo "  GAMDL:        ${MANIFEST_gamdl}"
    echo "  Votify:       ${MANIFEST_votify}"
    echo "  gytmdl:       ${MANIFEST_gytmdl}"
    echo "  yt-dlp:       ${MANIFEST_ytdlp}"
    echo "  --- Binary Tools ---"
    echo "  FFmpeg:       ${MANIFEST_ffmpeg}"
    echo "  mp4decrypt:   ${MANIFEST_mp4decrypt}"
    echo "  N_m3u8DL-RE:  ${MANIFEST_nm3u8dlre}"
    echo "  MP4Box:       ${MANIFEST_mp4box}"
    echo "  aria2c:       ${MANIFEST_aria2c}"
    echo "  fpcalc:       ${MANIFEST_fpcalc}"
    echo "  amdecrypt:    ${MANIFEST_amdecrypt}"
    echo "  --- Perl + get_iplayer ---"
    echo "  Perl:         ${MANIFEST_perl}"
    echo "  get_iplayer:  ${MANIFEST_get_iplayer}"
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
