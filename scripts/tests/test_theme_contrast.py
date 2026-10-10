"""Readability regressions for the default themes at full window opacity."""

from pathlib import Path
import re
import unittest


THEMES = Path(__file__).resolve().parents[2] / "assets" / "themes"


def luminance(color):
    channels = [int(color[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    linear = [c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
              for c in channels]
    return sum(c * weight for c, weight in zip(linear, (0.2126, 0.7152, 0.0722)))


class ThemeContrastTests(unittest.TestCase):
    def test_default_theme_text_pairs_have_readable_contrast(self):
        pairs = [
            ("foreground", "background"),
            ("secondary_foreground", "secondary"),
            ("popover_foreground", "popover"),
            ("accent_foreground", "accent"),
            ("danger_foreground", "danger"),
        ]
        pairs += [("primary_foreground", surface)
                  for surface in ("primary", "primary_hover", "primary_active")]
        pairs += [("muted_foreground", surface)
                  for surface in ("background", "secondary", "accent", "list_active")]
        for name in ("ropy-light", "ropy-dark"):
            palette = dict(re.findall(r'(\w+) = "(#[0-9a-fA-F]{6})"',
                                      (THEMES / f"{name}.toml").read_text()))
            for foreground, background in pairs:
                with self.subTest(theme=name, foreground=foreground, background=background):
                    low, high = sorted((luminance(palette[foreground]),
                                        luminance(palette[background])))
                    self.assertGreaterEqual((high + 0.05) / (low + 0.05), 4.5)


if __name__ == "__main__":
    unittest.main()
