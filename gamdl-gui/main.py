from PyQt5.QtWidgets import QApplication
from .ui import MainWindow
from .utils import download_library, is_library_available
from .constants import DEFAULT_LIBRARY_PATHS

def ensure_libraries_installed():
    """
    Ensures all required libraries are installed and functional.
    """
    for name, path in DEFAULT_LIBRARY_PATHS.items():
        if not is_library_available(path):
            print(f"{name} not found. Installing...")
            download_library(name)

def main():
    """
    Starts the application.
    """
    ensure_libraries_installed()

    app = QApplication([])
    window = MainWindow(ensure_libraries_installed, lambda: print("Checking for updates..."))
    window.show()
    app.exec_()

if __name__ == "__main__":
    main()