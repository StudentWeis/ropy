"""Exercise build-script target guards without building native GUI dependencies."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


@unittest.skipIf(os.name == "nt", "standalone Windows build script requires Cargo build dependencies")
class BuildScriptTests(unittest.TestCase):
    def test_build_script_rejects_cross_os_windows_before_resource_work(self):
        with tempfile.TemporaryDirectory() as tmp:
            executable = Path(tmp) / "build-script"
            subprocess.run(["rustc", "--edition=2024", str(ROOT / "build.rs"), "-o", str(executable)],
                           check=True, capture_output=True, text=True)
            for host, target, target_os, accepted in [
                ("aarch64-apple-darwin", "x86_64-pc-windows-msvc", "windows", False),
                ("x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu", "linux", False),
                ("aarch64-apple-darwin", "custom-target", "windows", False),
                ("aarch64-apple-darwin", "x86_64-apple-darwin", "macos", True),
                ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "linux", True),
            ]:
                with self.subTest(host=host, target=target):
                    result = subprocess.run([str(executable)], cwd=tmp,
                                            env=dict(os.environ, HOST=host, TARGET=target, CARGO_CFG_TARGET_OS=target_os),
                                            capture_output=True, text=True)
                    if accepted:
                        self.assertEqual(result.returncode, 0, result.stderr)
                        self.assertIn(f"cargo:rustc-env=TARGET={target}", result.stdout)
                    else:
                        self.assertNotEqual(result.returncode, 0)
                        self.assertIn("cross-OS Windows builds are not supported", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
