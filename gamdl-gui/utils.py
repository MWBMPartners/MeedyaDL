import os
import requests
import zipfile
from io import BytesIO

from .constants import GAMDL_RELEASE_URL, GITHUB_API_RELEASE_URL, DEFAULT_GAMDL_DIR

def download_gamdl(output_dir):
    """
    Downloads the gamdl release ZIP file and extracts it to the specified output directory.
    """
    try:
        response = requests.get(GAMDL_RELEASE_URL)
        response.raise_for_status()

        with zipfile.ZipFile(BytesIO(response.content)) as zf:
            zf.extractall(output_dir)
        return True
    except Exception as e:
        print(f"Error downloading gamdl: {e}")
        return False

def check_gamdl_update():
    """
    Checks for the latest version of gamdl using the GitHub API.
    Returns the version tag if available.
    """
    try:
        response = requests.get(GITHUB_API_RELEASE_URL)
        response.raise_for_status()
        release_info = response.json()
        return release_info.get("tag_name", None)
    except Exception as e:
        print(f"Error checking for updates: {e}")
        return None

def ensure_directory_exists(path):
    """
    Ensures that the specified directory exists. Creates it if it doesn't.
    """
    if not os.path.exists(path):
        os.makedirs(path)