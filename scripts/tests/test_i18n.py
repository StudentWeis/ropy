"""Regression tests for translation references in the lightweight gate."""

from pathlib import Path
import subprocess
import tempfile
import unittest


CHECKER = Path(__file__).resolve().parents[1] / "check" / "check_i18n.py"


class I18nTests(unittest.TestCase):
    def check_source(self, source, english='known = "Known"\n', chinese=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            locales = root / "assets" / "locales"
            locales.mkdir(parents=True)
            (root / "src").mkdir()
            for name in ("en", "zh-CN"):
                (locales / f"{name}.toml").write_text(english if name == "en" or chinese is None else chinese, encoding="utf-8")
            (root / "src" / "main.rs").write_text(source, encoding="utf-8")
            return subprocess.run(
                ["python3", str(CHECKER), "--root", str(root)],
                capture_output=True, text=True, check=False,
            )

    def test_i18n_unknown_translation_reference_fails(self):
        for reference in (
            't!("missing_key")',
            'rust_i18n::t!("missing_key", count = 3)',
            't!(\n"missing_key", locale = "ja")',
            'label_key: "missing_key"',
        ):
            with self.subTest(reference=reference):
                result = self.check_source('t!("known"); ' + reference)
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn("missing_key", result.stdout)

    def test_i18n_unrelated_map_lookup_does_not_require_translation(self):
        result = self.check_source('t!("known"); map.get("not_a_translation");')
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_i18n_unrelated_map_lookup_does_not_hide_unused_translation(self):
        result = self.check_source('map.get("known");')
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("UNUSED", result.stdout)

    def test_i18n_locale_placeholder_mismatch_fails(self):
        result = self.check_source('t!("known", count = 3)',
                                   'known = "%{count} files"\n',
                                   'known = "%{total} 个文件"\n')
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("placeholder", result.stdout.lower())

    def test_i18n_matching_placeholders_pass(self):
        result = self.check_source('t!("known", count = 3)',
                                   'known = "%{count} files"\n',
                                   'known = "%{count} 个文件"\n')
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_i18n_invalid_toml_fails(self):
        result = self.check_source('t!("known")', 'known = "unterminated\n')
        self.assertNotEqual(result.returncode, 0, result.stdout)


if __name__ == "__main__":
    unittest.main()
