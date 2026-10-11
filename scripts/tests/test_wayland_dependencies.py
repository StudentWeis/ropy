"""Keep Linux clipboard and input backends usable with native Wayland clients."""
import pathlib
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


class WaylandDependenciesTest(unittest.TestCase):
    def test_linux_clipboard_enables_wayland(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        dependency = manifest["target"]['cfg(target_os = "linux")']["dependencies"].get("clipboard-rs", {})
        self.assertIn("wayland", dependency.get("features", []))

    def test_linux_input_enables_wayland(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        dependency = manifest["target"]['cfg(target_os = "linux")']["dependencies"].get("enigo", {})
        self.assertIn("wayland", dependency.get("features", []))
