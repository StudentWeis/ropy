"""Verify that precheck stops early and its CI phases preserve the local gate."""

import os
import shutil
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
FORMAT_COMMAND = "cargo fmt --all --check"


class PrecheckTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)
        self.log = self.work / "commands"
        self.env = dict(
            os.environ,
            PATH=f"{self.work}:/usr/bin:/bin",
            PRECHECK_TEST_LOG=str(self.log),
            PRECHECK_FAIL_COMMAND="",
        )
        for name in ("cargo", "python3", "shfmt", "dbus-run-session"):
            command = self.work / name
            command.write_text('''#!/bin/bash
set -eu
name="${0##*/}"
printf '%s %s\\n' "$name" "$*" >> "$PRECHECK_TEST_LOG"
if [[ "$name $*" == "$PRECHECK_FAIL_COMMAND" ]]; then
    exit 17
fi
''')
            command.chmod(0o755)

        (self.work / "uname").write_text('#!/bin/sh\necho "${PRECHECK_TEST_OS:-Linux}"\n')
        (self.work / "uname").chmod(0o755)

    def test_precheck_linux_tray_failure_blocks_later_checks(self):
        tray = "dbus-run-session -- cargo test --locked -p ropy test_tray_without_gtk_initializes_refreshes_and_drops -- --ignored"
        result, commands = self.run_precheck("--check", "--rust", fail_command=tray)
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertIn(tray, commands)
        self.assertFalse(any("cargo doc" in command for command in commands))

    def test_precheck_other_platforms_do_not_require_dbus(self):
        for platform in ("Darwin", "MINGW64_NT"):
            with self.subTest(platform=platform):
                self.env["PRECHECK_TEST_OS"] = platform
                self.log.unlink(missing_ok=True)
                result, commands = self.run_precheck("--check", "--rust")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertFalse(any("dbus-run-session" in command for command in commands))

    def test_precheck_linux_missing_dbus_fails_explicitly(self):
        (self.work / "dbus-run-session").unlink()
        for name in ("bash", "dirname", "cat"):
            (self.work / name).symlink_to(shutil.which(name))
        self.env["PATH"] = str(self.work)
        result, _ = self.run_precheck("--check", "--rust")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("requires dbus-run-session", result.stderr)

    def run_precheck(self, *args, fail_command=""):
        self.env["PRECHECK_FAIL_COMMAND"] = fail_command
        result = subprocess.run(
            ["bash", str(ROOT / "scripts/precheck.sh"), *args],
            cwd=ROOT, env=self.env, capture_output=True, text=True,
        )
        commands = self.log.read_text().splitlines() if self.log.exists() else []
        return result, commands

    def test_precheck_format_drift_stops_before_compilation(self):
        result, commands = self.run_precheck(
            "--check", fail_command=FORMAT_COMMAND
        )
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertEqual(commands, [FORMAT_COMMAND])

    def test_precheck_resource_failure_stops_before_compilation(self):
        result, commands = self.run_precheck(
            "--check", fail_command="python3 scripts/check/check_i18n.py"
        )
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertFalse(any("clippy" in c or "cargo test" in c for c in commands))

    def test_precheck_light_phase_avoids_compilation(self):
        result, commands = self.run_precheck("--check", "--light")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(FORMAT_COMMAND, commands)
        self.assertIn("python3 scripts/check/check_icons.py", commands)
        self.assertIn("python3 scripts/check/check_themes.py", commands)
        self.assertIn("python3 -m unittest discover -s scripts/tests", commands)
        self.assertFalse(any(".agents/skills/" in c for c in commands))
        self.assertFalse(any("clippy" in c or "cargo test" in c for c in commands))
        self.assertFalse(any(c.startswith("shfmt -w") for c in commands))

    def test_precheck_script_failure_stops_before_compilation(self):
        result, commands = self.run_precheck(
            "--check", fail_command="python3 -m unittest discover -s scripts/tests"
        )
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertFalse(any("clippy" in command or "cargo test" in command for command in commands))

    def test_precheck_split_phases_match_full_ci_gate(self):
        result, full = self.run_precheck("--check")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.log.unlink()
        result, light = self.run_precheck("--check", "--light")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.log.unlink()
        result, rust = self.run_precheck("--check", "--rust")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(full, light + rust)
        self.assertIn("cargo clippy --locked --workspace --all-targets --all-features", rust)
        self.assertIn("cargo test --locked --workspace --all-targets --all-features", rust)
        self.assertIn("cargo doc --locked --workspace --no-deps", rust)
        self.assertFalse(any(c.startswith("cargo check") for c in full))

    def test_precheck_invalid_arguments_fail_before_running_checks(self):
        for args in (("--unknown",), ("--light", "--rust")):
            with self.subTest(args=args):
                result, commands = self.run_precheck(*args)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(commands, [])


if __name__ == "__main__":
    unittest.main()
