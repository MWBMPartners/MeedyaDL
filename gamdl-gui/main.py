import os
from PyQt5.QtWidgets import QApplication
from .ui import MainWindow
from .utils import download_gamdl, check_gamdl_update, ensure_directory_exists
from .constants import DEFAULT_GAMDL_DIR

def install_gamdl():
    """
    Installs gamdl by downloading and extracting it to the default directory.
    """
    ensure_directory_exists(DEFAULT_GAMDL_DIR)
    if download_gamdl(DEFAULT_GAMDL_DIR):
        print("gamdl installed successfully!")
    else:
        print("Failed to install gamdl.")

def check_for_updates():
    """
    Checks for updates to gamdl and notifies the user.
    """
    latest_version = check_gamdl_update()
    if latest_version:
        print(f"Latest version: {latest_version}")
    else:
        print("Failed to fetch the latest version.")

def main():
    """
    Main function to start the application.
    """
    app = QApplication([])

    window = MainWindow(install_gamdl, check_for_updates)
    window.show()

    app.exec_()

if __name__ == "__main__":
    main()