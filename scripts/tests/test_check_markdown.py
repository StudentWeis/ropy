"""Regression tests for the Markdown link and anchor checker."""

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts" / "check" / "check_markdown.py"


class MarkdownTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "docs").mkdir()
        (self.root / "docs" / "guide.md").write_text(
            "# Guide\n\n## Recovery after a load error\n\nText.\n", encoding="utf-8"
        )

    def check(self, source: str, path: str = "README.md") -> subprocess.CompletedProcess:
        (self.root / path).write_text(source, encoding="utf-8")
        return subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(self.root)],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_existing_file_and_heading_anchor_pass(self):
        result = self.check(
            "[guide](docs/guide.md) and [recovery](docs/guide.md#recovery-after-a-load-error)\n"
            "and [same file](#top)\n\n# Top\n"
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("links and anchors resolve", result.stdout)

    def test_non_markdown_target_anchor_is_not_parsed(self):
        (self.root / "assets").mkdir()
        (self.root / "assets" / "data.json").write_text("{}\n", encoding="utf-8")
        result = self.check("[data](assets/data.json#field)\n")
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_missing_file_fails(self):
        result = self.check("[guide](docs/missing.md)\n")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("docs/missing.md does not exist", result.stdout)

    def test_missing_heading_fails(self):
        result = self.check("[guide](docs/guide.md#absent-heading)\n")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("no matching heading", result.stdout)

    def test_examples_in_code_are_ignored(self):
        result = self.check(
            "```md\n[broken](docs/missing.md)\n```\n\nInline `[broken](docs/missing.md)` too.\n"
        )
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_external_and_site_relative_targets_are_ignored(self):
        result = self.check(
            "[site](https://example.com) [mail](mailto:a@example.com)\n"
            "[upstream](/docs/guide) [protocol](//example.com/x)\n"
        )
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_paths_escaping_the_repository_fail(self):
        result = self.check("[escape](../../outside.md)\n")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("escapes the repository", result.stdout)

    def test_vendored_gpui_kit_snapshots_are_skipped(self):
        snapshot = self.root / ".agents" / "skills" / "gpui-kit" / "SKILL.md"
        snapshot.parent.mkdir(parents=True)
        snapshot.write_text("[upstream](./missing.md)\n", encoding="utf-8")
        result = self.check("[fine](docs/guide.md)\n")
        self.assertEqual(result.returncode, 0, result.stdout)


if __name__ == "__main__":
    unittest.main()
