import pytest
from gamdl_gui.utils import ensure_directory_exists

def test_ensure_directory_exists(tmp_path):
    """
    Test that ensure_directory_exists creates a directory if it doesn't exist.
    """
    test_dir = tmp_path / "test_dir"
    ensure_directory_exists(test_dir)
    assert test_dir.exists()
