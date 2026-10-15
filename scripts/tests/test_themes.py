"""Regression tests for the theme checker's runtime parsing contract."""

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts" / "check" / "check_themes.py"
VALID_THEME = (ROOT / "assets" / "themes" / "ropy-light.toml").read_text(encoding="utf-8")


class ThemeTests(unittest.TestCase):
    def check_theme(self, source, template=VALID_THEME):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            themes = root / "assets" / "themes"
            themes.mkdir(parents=True)
            (themes / "ropy-light.toml").write_text(template, encoding="utf-8")
            (themes / "custom.toml").write_text(source, encoding="utf-8")
            return subprocess.run(
                [sys.executable, str(CHECKER), "--root", str(root)],
                capture_output=True, text=True, check=False,
            )

    def test_theme_runtime_color_formats(self):
        for value in ('"#abc"', '"#11223344"', '"#12gh56"', '42', '""'):
            with self.subTest(value=value):
                result = self.check_theme(VALID_THEME.replace('background = "#ffffff"', f'background = {value}'))
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn("background", result.stdout)

    def test_theme_valid_toml_and_runtime_rgb_variants_pass(self):
        for value in ("'#aBcDeF' # comment", '"  ABCdef  "', '"#123456"'):
            with self.subTest(value=value):
                result = self.check_theme(VALID_THEME.replace('background = "#ffffff"', f'background = {value}'))
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_theme_invalid_mode_fails(self):
        for value in ('"sepia"', '"Light"', 'true', '["light"]'):
            with self.subTest(value=value):
                result = self.check_theme(VALID_THEME.replace('mode = "light"', f'mode = {value}'))
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn("mode", result.stdout)

    def test_theme_invalid_toml_reports_file_without_traceback(self):
        for source in (VALID_THEME + 'background = "#123456"\n', VALID_THEME + 'broken = "unterminated\n'):
            with self.subTest(source=source):
                result = self.check_theme(source)
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn("custom.toml", result.stdout)
                self.assertIn("TOML", result.stdout)
                self.assertNotIn("Traceback", result.stderr)

    def test_theme_schema_requires_fields_even_when_template_omits_them(self):
        for field in ("background", "mode", "theme_name"):
            source = "\n".join(line for line in VALID_THEME.splitlines() if not line.startswith(field + " ="))
            with self.subTest(field=field):
                result = self.check_theme(source, template=source)
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(field, result.stdout)

    def test_theme_metadata_requires_string_name(self):
        source = VALID_THEME.replace('theme_name = "Ropy Light"', 'theme_name = 42')
        result = self.check_theme(source)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("theme_name", result.stdout)

    def test_theme_extra_keys_remain_rejected(self):
        result = self.check_theme(VALID_THEME + 'unexpected = "#123456"\n')
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("unexpected", result.stdout)


if __name__ == "__main__":
    unittest.main()
