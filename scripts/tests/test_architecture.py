"""Keep core modules independent of desktop adapters across the workspace."""

from pathlib import Path
import re
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]


class ArchitectureTests(unittest.TestCase):
    def assert_dependencies_exclude(self, paths, forbidden):
        for path in paths:
            source = path.read_text()
            imports = "\n".join(re.findall(r"\buse\s+([^;]+);", source))
            for dependency in forbidden:
                with self.subTest(path=str(path.relative_to(ROOT)), dependency=dependency):
                    self.assertNotRegex(imports, rf"\b{dependency}\b")
                    self.assertNotRegex(source, rf"\b{dependency}\s*::")

    def test_repository_dependencies_exclude_desktop_and_clipboard(self):
        paths = [ROOT / "crates/ropy-core/src/repository.rs", *sorted((ROOT / "crates/ropy-core/src/repository").rglob("*.rs"))]
        self.assert_dependencies_exclude(
            paths, ("gpui_kit", "clipboard_rs", "gui", "clipboard", "app", "i18n", "config", "utils")
        )

    def test_settings_dependencies_exclude_presentation(self):
        paths = [ROOT / "crates/ropy-core/src/config/settings.rs"]
        paths.extend((ROOT / "crates/ropy-core/src/config").glob("*_id.rs"))
        self.assert_dependencies_exclude(paths, ("gpui_kit", "gui", "i18n", "rust_embed"))

    def test_clipboard_dependencies_exclude_gpui_runtime(self):
        paths = [ROOT / "src/clipboard.rs", *sorted((ROOT / "src/clipboard").rglob("*.rs"))]
        self.assert_dependencies_exclude(paths, ("gpui_kit", "gui", "app"))

    def test_core_is_a_workspace_member_with_shared_lints(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        self.assertIn("crates/ropy-core", manifest["workspace"]["members"])
        core = tomllib.loads((ROOT / "crates/ropy-core/Cargo.toml").read_text())
        self.assertTrue(core["lints"]["workspace"])
        self.assertFalse(core["package"]["publish"])
        self.assertFalse(core["package"]["metadata"]["release"]["release"])
