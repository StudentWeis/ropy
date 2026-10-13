"""Compile the block patch directly so Cargo cannot cap its compatibility lint."""

from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class BlockCompatibilityTests(unittest.TestCase):
    def test_block_ffi_symbols_compile_with_uninhabited_static_denied(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                [
                    "rustc", "--edition=2015", "--crate-name", "block",
                    "--crate-type", "rlib", "-D", "warnings",
                    "-D", "uninhabited_static",
                    str(ROOT / "vendor/block/src/lib.rs"),
                    "--out-dir", directory,
                ],
                capture_output=True, text=True, check=False,
            )
        self.assertEqual(result.returncode, 0, result.stderr)
