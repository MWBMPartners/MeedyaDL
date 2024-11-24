from PyQt5.QtWidgets import QVBoxLayout, QLabel, QPushButton, QWidget

class MainWindow(QWidget):
    """
    Main application window.
    """
    def __init__(self, install_callback, update_callback):
        super().__init__()
        self.setWindowTitle("Gamdl GUI")
        self.setGeometry(100, 100, 400, 300)

        self.init_ui(install_callback, update_callback)

    def init_ui(self, install_callback, update_callback):
        layout = QVBoxLayout()

        self.status_label = QLabel("Checking dependencies...")
        layout.addWidget(self.status_label)

        self.install_button = QPushButton("Install Missing Libraries")
        self.install_button.clicked.connect(install_callback)
        layout.addWidget(self.install_button)

        self.update_button = QPushButton("Check for Updates")
        self.update_button.clicked.connect(update_callback)
        layout.addWidget(self.update_button)

        self.setLayout(layout)