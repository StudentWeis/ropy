"""Verify that precheck stops early and its CI phases preserve the local gate."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


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
        for name in ("cargo", "python3", "cargo-machete", "shfmt"):
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
            "--check", fail_command="cargo +nightly fmt --check"
        )
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertEqual(commands, ["cargo +nightly fmt --check"])

    def test_precheck_resource_failure_stops_before_compilation(self):
        result, commands = self.run_precheck(
            "--check", fail_command="python3 scripts/check/check_i18n.py"
        )
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertFalse(any("clippy" in c or "cargo test" in c for c in commands))

    def test_precheck_light_phase_avoids_compilation(self):
        result, commands = self.run_precheck("--check", "--light")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("cargo +nightly fmt --check", commands)
        self.assertIn("cargo machete", commands)
        self.assertIn("python3 scripts/check/check_icons.py", commands)
        self.assertIn("python3 scripts/check/check_themes.py", commands)
        self.assertIn("python3 -m unittest discover -s scripts/tests", commands)
        self.assertIn("python3 -m unittest discover -s .agents/skills/repo-coordination/tests", commands)
        self.assertFalse(any("clippy" in c or "cargo test" in c for c in commands))
        self.assertFalse(any(c.startswith("shfmt -w") for c in commands))

    def test_precheck_coordination_failure_stops_before_compilation(self):
        result, commands = self.run_precheck(
            "--check", fail_command="python3 -m unittest discover -s .agents/skills/repo-coordination/tests"
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
        self.assertIn("cargo clippy --all-targets --all-features", rust)
        self.assertIn("cargo test --all-targets --all-features", rust)
        self.assertIn("cargo doc --no-deps", rust)
        self.assertFalse(any(c.startswith("cargo check") for c in full))

    def test_precheck_invalid_arguments_fail_before_running_checks(self):
        for args in (("--unknown",), ("--light", "--rust")):
            with self.subTest(args=args):
                result, commands = self.run_precheck(*args)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(commands, [])


if __name__ == "__main__":
    unittest.main()
