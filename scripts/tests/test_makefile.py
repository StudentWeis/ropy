"""Developer check targets must execute across both workspace packages."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class MakefileTests(unittest.TestCase):
    def test_check_targets_include_core_and_preserve_cargo_override(self):
        with tempfile.TemporaryDirectory() as directory:
            command = Path(directory) / "cargo"
            command.write_text('#!/bin/sh\nprintf "%s\\n" "$*"\n')
            command.chmod(0o755)
            for target in ("check", "test", "clippy", "doc", "fmt", "fmt-check"):
                with self.subTest(target=target):
                    result = subprocess.run(
                        ["make", "--no-print-directory", target, f"CARGO={command}"],
                        cwd=ROOT, env=os.environ, capture_output=True, text=True,
                        check=False,
                    )
                    self.assertEqual(result.returncode, 0, result.stderr)
                    arguments = result.stdout.splitlines()[-1].split()
                    self.assertIn("--all" if target.startswith("fmt") else "--workspace", arguments)


if __name__ == "__main__":
    unittest.main()
