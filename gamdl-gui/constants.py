# constants.py

# URLs for downloading third-party libraries
LIBRARY_URLS = {
    "FFmpeg": "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip",
    "MP4Box": "https://example.com/mp4box.zip",  # Replace with actual URL
    "yt-dlp": "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
    "N_m3u8DL-RE": "https://github.com/nilaoda/N_m3u8DL-RE/releases/latest/download/N_m3u8DL-RE.exe"
}

# Default directory for storing libraries
LIBRARY_INSTALL_DIR = "./bin"

# Paths for libraries (can be overridden by user settings)
DEFAULT_LIBRARY_PATHS = {
    "FFmpeg": f"{LIBRARY_INSTALL_DIR}/ffmpeg",
    "MP4Box": f"{LIBRARY_INSTALL_DIR}/MP4Box",
    "yt-dlp": f"{LIBRARY_INSTALL_DIR}/yt-dlp",
    "N_m3u8DL-RE": f"{LIBRARY_INSTALL_DIR}/N_m3u8DL-RE"
}