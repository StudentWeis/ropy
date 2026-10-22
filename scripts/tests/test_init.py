"""Check setup tool selection without installing tools or Git hooks."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class SetupTests(unittest.TestCase):
    def test_setup_installs_configured_dist_from_its_repository(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory).resolve()
            repository = work / "repository"
            scripts = repository / "scripts"
            scripts.mkdir(parents=True)
            shutil.copyfile(ROOT / "scripts/init.sh", scripts / "init.sh")
            commands = work / "bin"
            commands.mkdir()
            log = work / "commands"
            for name, body in {
                "cargo": 'printf "%s\\n" "$*" >> "$INIT_TEST_LOG"',
                "hk": 'printf "hooks in %s\\n" "$PWD" >> "$INIT_TEST_LOG"',
                "uvx": "exit 0",
            }.items():
                command = commands / name
                command.write_text("#!/bin/sh\nset -eu\n" + body + "\n")
                command.chmod(0o755)
            env = dict(os.environ, PATH=f"{commands}:{os.environ['PATH']}", INIT_TEST_LOG=str(log))
            for version in ("0.32.0", "9.9.9"):
                with self.subTest(version=version):
                    (repository / "dist-workspace.toml").write_text(
                        f'[dist]\ncargo-dist-version = "{version}"\n'
                    )
                    log.unlink(missing_ok=True)
                    result = subprocess.run(
                        ["bash", str(scripts / "init.sh")], cwd=work, env=env,
                        capture_output=True, text=True,
                    )
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(log.read_text().splitlines(), [
                        "install cargo-binstall",
                        f"binstall hk@^2.5 cargo-dist@{version} cargo-release git-cliff cargo-outdated",
                        f"hooks in {repository}",
                    ])
