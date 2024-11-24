import os
import requests
import zipfile
from io import BytesIO
import subprocess
from .constants import LIBRARY_URLS, LIBRARY_INSTALL_DIR

def download_library(name):
    """
    Downloads the specified library and extracts or saves it to the bin directory.
    """
    url = LIBRARY_URLS[name]
    output_dir = LIBRARY_INSTALL_DIR

    # Ensure the directory exists
    os.makedirs(output_dir, exist_ok=True)

    response = requests.get(url)
    if response.status_code == 200:
        if url.endswith(".zip"):
            with zipfile.ZipFile(BytesIO(response.content)) as zf:
                zf.extractall(output_dir)
        else:
            binary_path = os.path.join(output_dir, f"{name}.exe" if os.name == "nt" else name)
            with open(binary_path, "wb") as file:
                file.write(response.content)
        print(f"{name} downloaded successfully.")
        return True
    else:
        print(f"Failed to download {name}. HTTP Status: {response.status_code}")
        return False

def is_library_available(path):
    """
    Checks if the library at the given path is functional.
    """
    try:
        subprocess.run([path, "--version"], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        return True
    except Exception as e:
        print(f"Library check failed for {path}: {e}")
        return False

def check_library_update(name):
    """
    Checks if a newer version of the library is available.
    """
    url = LIBRARY_URLS[name]
    response = requests.head(url)
    return response.status_code == 200