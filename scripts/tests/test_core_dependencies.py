"""Keep the standalone core check fail-closed for transitive desktop dependencies."""

import importlib.util
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "check/check_core_dependencies.py"
SPEC = importlib.util.spec_from_file_location("core_dependencies", SCRIPT)
CHECK = importlib.util.module_from_spec(SPEC)
with patch("sys.dont_write_bytecode", True):
    SPEC.loader.exec_module(CHECK)


class CoreDependencyTests(unittest.TestCase):
    def run_check(self, output, returncode=0):
        result = subprocess.CompletedProcess([], returncode, stdout=output, stderr="")
        with patch.object(CHECK.subprocess, "run", return_value=result) as run:
            with patch("builtins.print"):
                code = CHECK.main()
        args = run.call_args.args[0]
        self.assertIn("--all-features", args)
        self.assertEqual(args[args.index("--target") + 1], "all")
        self.assertEqual(args[args.index("--edges") + 1], "normal,build,dev")
        return code

    def test_core_dependencies_transitive_native_packages_are_rejected(self):
        for name in ("gpui-kit", "gpui-component", "gtk-sys", "global-hotkey", "clipboard-rs"):
            with self.subTest(package=name):
                self.assertEqual(self.run_check(f"ropy-core v0.1.0\n{name} v1.0.0\n"), 1)

    def test_core_dependencies_storage_and_platform_path_helpers_are_allowed(self):
        self.assertEqual(self.run_check("ropy-core v0.1.0\nredb v4.3.0\ndirs v7.0.0\n"), 0)

    def test_core_dependencies_cargo_failure_is_not_reported_as_success(self):
        self.assertEqual(self.run_check("", returncode=101), 101)
