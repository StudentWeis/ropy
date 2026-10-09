"""Regression tests for translation references in the lightweight gate."""

from pathlib import Path
import subprocess
import tempfile
import unittest


CHECKER = Path(__file__).resolve().parents[1] / "check" / "check_i18n.py"


class I18nTests(unittest.TestCase):
    def check_source(self, source):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            locales = root / "assets" / "locales"
            locales.mkdir(parents=True)
            (root / "src").mkdir()
            for name in ("en", "zh-CN"):
                (locales / f"{name}.toml").write_text('known = "Known"\n', encoding="utf-8")
            (root / "src" / "main.rs").write_text(source, encoding="utf-8")
            return subprocess.run(
                ["python3", str(CHECKER), "--root", str(root)],
                capture_output=True, text=True, check=False,
            )

    def test_i18n_unknown_translation_reference_fails(self):
        for reference in (
            'I18n::translate(cx, "missing_key")',
            'I18n::translate_count(cx, "missing_key", 3)',
            'i18n.t("missing_key")',
            'label_key: "missing_key"',
        ):
            with self.subTest(reference=reference):
                result = self.check_source('I18n::translate(cx, "known"); ' + reference)
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn("missing_key", result.stdout)

    def test_i18n_unrelated_map_lookup_does_not_require_translation(self):
        result = self.check_source('I18n::translate(cx, "known"); map.get("not_a_translation");')
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_i18n_unrelated_map_lookup_does_not_hide_unused_translation(self):
        result = self.check_source('map.get("known");')
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("UNUSED", result.stdout)


if __name__ == "__main__":
    unittest.main()
