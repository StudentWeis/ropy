"""Check setup tool selection without installing tools or Git hooks."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class SetupTests(unittest.TestCase):
    def setUp(self):
        self.addCleanup(self._cleanup)
        self.directory = tempfile.TemporaryDirectory()
        self.work = Path(self.directory.name).resolve()
        self.repository = self.work / "repository"
        self.scripts = self.repository / "scripts"
        self.scripts.mkdir(parents=True)
        shutil.copyfile(ROOT / "scripts/init.sh", self.scripts / "init.sh")
        commands = self.work / "bin"
        commands.mkdir()
        self.log = self.work / "commands"
        for name, body in {
            "cargo": 'printf "%s\\n" "$*" >> "$INIT_TEST_LOG"',
            "hk": 'printf "hooks in %s\\n" "$PWD" >> "$INIT_TEST_LOG"',
            "uvx": "exit 0",
        }.items():
            command = commands / name
            command.write_text("#!/bin/sh\nset -eu\n" + body + "\n")
            command.chmod(0o755)
        self.env = dict(
            os.environ,
            PATH=f"{commands}:/usr/bin:/bin",
            INIT_TEST_LOG=str(self.log),
        )

    def _cleanup(self):
        self.directory.cleanup()

    def run_setup(self, dist_version="0.32.0", hk_version="9.9.9"):
        (self.repository / "dist-workspace.toml").write_text(
            f'[dist]\ncargo-dist-version = "{dist_version}"\n'
        )
        (self.repository / "hk.pkl").write_text(
            f'amends "package://github.com/jdx/hk/releases/download/v{hk_version}/hk@'
            f'{hk_version}#/Config.pkl"\n\nmin_hk_version = "{hk_version}"\n'
        )
        self.log.unlink(missing_ok=True)
        return subprocess.run(
            ["bash", str(self.scripts / "init.sh")],
            cwd=self.work,
            env=self.env,
            capture_output=True,
            text=True,
        )

    def test_setup_installs_configured_dist_from_its_repository(self):
        for dist_version, hk_version in (("0.32.0", "9.9.9"), ("9.9.9", "8.8.8")):
            with self.subTest(dist_version=dist_version, hk_version=hk_version):
                result = self.run_setup(dist_version, hk_version)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.log.read_text().splitlines(), [
                    "install cargo-binstall",
                    f"binstall hk@{hk_version} cargo-dist@{dist_version} "
                    "cargo-release git-cliff cargo-outdated",
                    f"hooks in {self.repository}",
                ])

    def test_setup_rejects_hk_config_without_version(self):
        (self.repository / "dist-workspace.toml").write_text(
            '[dist]\ncargo-dist-version = "0.32.0"\n'
        )
        (self.repository / "hk.pkl").write_text("fail_fast = true\n")
        self.log.unlink(missing_ok=True)

        result = subprocess.run(
            ["bash", str(self.scripts / "init.sh")],
            cwd=self.work,
            env=self.env,
            capture_output=True,
            text=True,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("min_hk_version", result.stderr)
        self.assertFalse(self.log.exists())
