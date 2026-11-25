"""Regression tests for the vendored agent skill snapshot checker."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts" / "check" / "check_agent_skills.py"
COMMIT = "39ab849a93b5f15507b969164d49891db8d5b470"
CARGO_TOML = """[dependencies]
gpui-kit = { version = "0.9.1", default-features = false, features = ["component"] }
[dev-dependencies]
gpui-kit = { version = "0.9.1", features = ["test-support"] }
"""
CARGO_LOCK = """version = 4

[[package]]
name = "gpui-kit"
version = "0.9.1"

[[package]]
name = "gpui-pre"
version = "0.3.12"
"""
ADAPTER = f"""Ropy pins GPUI Kit 0.9.1 with the gpui-pre 0.3.12 engine at commit `{COMMIT}`.
"""


class AgentSkillTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "Cargo.toml").write_text(CARGO_TOML, encoding="utf-8")
        (self.root / "Cargo.lock").write_text(CARGO_LOCK, encoding="utf-8")
        self.write(".agents/skills/ropy-gpui/SKILL.md", ADAPTER)
        self.write(".agents/skills/gpui-kit/SKILL.md", "# GPUI Kit\n")
        self.write(".agents/skills/gpui-kit/references/usage.md", "Upstream usage.\n")
        self.write(".agents/skills/gpui-kit-design-guides/SKILL.md", "# Design\n")
        manifest = {
            name: {
                "commit": COMMIT,
                "path": f".agents/skills/{name}",
                "sha256": "",
                "tag": "v0.9.1",
            }
            for name in ("gpui-kit", "gpui-kit-design-guides")
        }
        self.write(".agents/official-snapshots.json", json.dumps(manifest))
        self.run_checker("--update")

    def write(self, relative: str, text: str) -> Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        return path

    def run_checker(self, *args: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(self.root), *args],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_unchanged_snapshots_and_recorded_versions_pass(self):
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("versions recorded in the adapter", result.stdout)

    def test_modified_snapshot_file_fails(self):
        self.write(".agents/skills/gpui-kit/SKILL.md", "# GPUI Kit\n\nLocal edit.\n")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("content differs from the recorded gpui-kit snapshot", result.stdout)

    def test_added_snapshot_file_fails(self):
        self.write(".agents/skills/gpui-kit/references/local-notes.md", "Local notes.\n")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("content differs", result.stdout)

    def test_recorded_revision_must_appear_in_the_adapter(self):
        self.write(".agents/skills/ropy-gpui/SKILL.md", ADAPTER.replace(COMMIT, "0" * 40))
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not record the gpui-kit commit", result.stdout)

    def test_declared_versions_must_appear_in_the_adapter(self):
        (self.root / "Cargo.toml").write_text(CARGO_TOML.replace("0.9.1", "0.9.2"), encoding="utf-8")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not record gpui-kit 0.9.2", result.stdout)

    def test_locked_engine_version_must_appear_in_the_adapter(self):
        self.write("Cargo.lock", CARGO_LOCK.replace("0.9.1", "0.9.2"))
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("locked gpui-kit", result.stdout)

    def test_locked_gpui_pre_version_must_appear_in_the_adapter(self):
        (self.root / "Cargo.lock").write_text(CARGO_LOCK.replace("0.3.12", "0.4.0"), encoding="utf-8")
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not record gpui-pre 0.4.0", result.stdout)

    def test_dev_dependency_drift_reports_the_mismatched_declaration(self):
        normal, dev = CARGO_TOML.split("[dev-dependencies]")
        self.write("Cargo.toml", normal + "[dev-dependencies]" + dev.replace("0.9.1", "0.9.0"))
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("dev-dependencies.gpui-kit version '0.9.0'", result.stdout)

    def test_update_accepts_a_replaced_snapshot(self):
        self.write(".agents/skills/gpui-kit/SKILL.md", "# GPUI Kit\n\nReplacement.\n")
        updated = self.run_checker("--update")
        self.assertEqual(updated.returncode, 0, updated.stdout)
        result = self.run_checker()
        self.assertEqual(result.returncode, 0, result.stdout)

    def test_missing_manifest_entry_fails(self):
        manifest = json.loads((self.root / ".agents" / "official-snapshots.json").read_text())
        del manifest["gpui-kit-design-guides"]
        self.write(".agents/official-snapshots.json", json.dumps(manifest))
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not record the gpui-kit-design-guides snapshot", result.stdout)

    def test_missing_snapshot_directory_fails(self):
        target = self.root / ".agents" / "skills" / "gpui-kit-design-guides"
        for path in sorted(target.rglob("*"), reverse=True):
            path.unlink()
        target.rmdir()
        result = self.run_checker()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("snapshot directory is missing", result.stdout)


if __name__ == "__main__":
    unittest.main()
