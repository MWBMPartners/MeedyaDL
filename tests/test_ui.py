import pytest
from PyQt5.QtWidgets import QApplication
from PyQt5.QtTest import QTest
from PyQt5.QtCore import Qt
from gamdl_gui.ui import MainWindow

@pytest.fixture
def app():
    """
    Create a Qt application instance for testing.
    """
    app = QApplication([])
    return app

@pytest.fixture
def main_window():
    """
    Create an instance of the MainWindow for testing.
    """
    def install_callback():
        print("Install button clicked!")

    def update_callback():
        print("Update button clicked!")

    window = MainWindow(install_callback, update_callback)
    return window

def test_main_window_ui(main_window, qtbot