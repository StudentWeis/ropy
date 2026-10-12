"""Behavioral tests for verified Homebrew Cask generation."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "generate_homebrew_cask.py"


class HomebrewTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("homebrew", SCRIPT)
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)

    def test_cask_uses_both_verified_architectures_and_managed_marker(self):
        with tempfile.TemporaryDirectory() as directory:
            assets = Path(directory)
            for arch in ("aarch64", "x86_64"):
                (assets / f"ropy-{arch}-apple-darwin.dmg").write_bytes(arch.encode())
            cask = self.module.render("0.5.7", assets)
            self.assertIn('arm: "aarch64", intel: "x86_64"', cask)
            self.assertIn(self.module.hashlib.sha256(b"aarch64").hexdigest(), cask)
            self.assertIn(self.module.hashlib.sha256(b"x86_64").hexdigest(), cask)
            self.assertIn('Contents/.ropy-managed', cask)
            self.assertNotIn('auto_updates true', cask)

    def test_cask_missing_architecture_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(FileNotFoundError):
                self.module.render("0.5.7", Path(directory))

    def test_cask_prerelease_and_injected_versions_fail(self):
        for version in ('0.5.8-beta.1', 'latest', '0.5.7"', 'v0.5.7'):
            with self.subTest(version=version), self.assertRaises(ValueError):
                self.module.render(version, Path("unused"))

    def test_release_missing_digest_fails_without_output(self):
        release = {"tagName": "0.5.7", "isDraft": False, "isPrerelease": False,
                   "assets": [{"name": "ropy-aarch64-apple-darwin.dmg", "digest": None}]}
        with self.assertRaises(ValueError):
            self.module.render_release(release)

    def test_release_prerelease_fails(self):
        with self.assertRaises(ValueError):
            self.module.render_release({"isDraft": False, "isPrerelease": True})

    def test_release_uses_published_digests(self):
        release = {"tagName": "0.5.7", "isDraft": False, "isPrerelease": False,
                   "assets": [{"name": f"ropy-{arch}-apple-darwin.dmg",
                               "digest": "sha256:" + digest * 64}
                              for arch, digest in [("aarch64", "a"), ("x86_64", "b")]]}
        content = self.module.render_release(release)
        self.assertIn("a" * 64, content)
        self.assertIn("b" * 64, content)
